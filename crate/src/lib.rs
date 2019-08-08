
use futures::Future;
use image::{GenericImage,GenericImageView};
use std::io::Cursor;
use std::convert::TryFrom;
use std::str::FromStr;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::{future_to_promise,JsFuture};

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[allow(unused_macros)]
macro_rules! console_log {
    ($($t:tt)*) => (web_sys::console::log_1(&format!($($t)*).into()))
}

#[wasm_bindgen]
pub fn init() {
    console_error_panic_hook::set_once();
}

#[wasm_bindgen]
pub struct WorkingImage {
    fut: Box<Future<Item = ImageInfo, Error = JsValue>>
}

pub struct ImageInfo {
    image: image::DynamicImage,
    headers: web_sys::Headers,
    #[allow(dead_code)]
    format: ImageFormat,
}

fn err_str_to_js(s: String) -> JsValue {
    JsValue::from(js_sys::Error::new(&s))
}

fn err_img_to_js(i: image::ImageError) -> JsValue {
    JsValue::from(js_sys::Error::new(&format!("Image error: {}", i)))
}

#[wasm_bindgen]
impl WorkingImage {
    #[wasm_bindgen(constructor)]
    pub fn new(url: &str, format: Option<ImageFormat>) -> WorkingImage {
        // Start fetch
        let global = js_sys::global().unchecked_into::<web_sys::WorkerGlobalScope>();
        let resp_promise = global.fetch_with_str(url);
        let fut = Box::new(JsFuture::from(resp_promise)
            // Read buffer if response is OK
            .and_then(|resp_val| {
                let resp: web_sys::Response = resp_val.dyn_into().unwrap();
                if !resp.ok() {
                    // Just throw the response on failure
                    return Err(JsValue::from(resp));
                }
                Ok((resp.headers(), resp.array_buffer().unwrap()))
            })
            // Extract buffer from promise
            .and_then(move |(headers, buf_promise)| {
                JsFuture::from(buf_promise).map(move |buf_val| { (headers, buf_val) })
            })
            // Copy buffer to byte array and load image
            .and_then(move |(headers, buf)| {
                let uint8_arr = js_sys::Uint8Array::new(&buf);
                let mut bytes = vec![0; uint8_arr.length() as usize];
                uint8_arr.copy_to(&mut bytes);
                let in_format: ImageFormat = match format {
                    Some(format) => format,
                    None => {
                        ImageFormat::try_from(
                            image::guess_format(&bytes).map_err(err_img_to_js)?).map_err(err_str_to_js)?
                    },
                };
                Ok(ImageInfo {
                    image: image::load_from_memory_with_format(&bytes, in_format.into())
                        .map_err(err_img_to_js)?,
                    headers: headers,
                    format: in_format,
                })
            }));
        WorkingImage { fut: fut }
    }

    pub fn build(self, format: Option<ImageFormat>) -> js_sys::Promise {
        future_to_promise(
            self.fut.and_then(move |info| {
                let out_format = format.unwrap_or(ImageFormat::JPEG);
                // We'll use the same headers, but remove length and set type
                let headers = web_sys::Headers::new_with_headers(&info.headers).unwrap();
                headers.delete("Content-Length").unwrap();
                headers.set("Content-Type", out_format.mime_type()).unwrap();
                // Write to a buffer
                let mut buf = Cursor::new(Vec::new());
                let out_image_format: image::ImageFormat = out_format.into();
                info.image.write_to(&mut buf, out_image_format).map_err(err_img_to_js)?;
                Ok((headers, buf))
            })
            .and_then(|(headers, buf)| {
                // Build the response
                let body = js_sys::Uint8Array::from(buf.get_ref().as_slice());
                let resp = web_sys::Response::new_with_opt_buffer_source_and_init(
                    Some(&body), web_sys::ResponseInit::new().headers(&headers))?;
                Ok(JsValue::from(resp))
            })
        )
    }

    pub fn blur(self, sigma: f32) -> WorkingImage {
        WorkingImage{
            fut: Box::new(self.fut.map(move |info| {
                ImageInfo { image: info.image.blur(sigma), ..info }
            })),
        }
    }

    pub fn border(
        self,
        top: f64, top_pct: bool,
        right: f64, right_pct: bool,
        bottom: f64, bottom_pct: bool,
        left: f64, left_pct: bool,
        color: Option<String>,
    ) -> WorkingImage {
        WorkingImage{
            fut: Box::new(self.fut.and_then(move |info| {
                // Get dimensions
                let (mut w, mut h) = info.image.dimensions();
                let top_val = val_or_pct(h, top, top_pct);
                let right_val = val_or_pct(w, right, right_pct);
                let bottom_val = val_or_pct(h, bottom, bottom_pct);
                let left_val = val_or_pct(w, left, left_pct);
                // New image with other copied in
                w += left_val + right_val;
                h += top_val + bottom_val;
                let mut new_image = image::DynamicImage::new_rgba8(w, h);
                new_image.copy_from(&info.image, left_val, top_val);
                // If there is a color, apply it to the borders
                if let Some(ref color_str) = color {
                    let rgba = rgba_string(color_str).map_err(err_str_to_js)?;
                    for x in 0..w {
                        for y in 0..h {
                            if x < left_val || x >= w - right_val || y < top_val || y >= h - bottom_val {
                                new_image.put_pixel(x, y, rgba);
                            }
                        }
                    }
                }
                Ok(ImageInfo { image: new_image, ..info })
            })),
        }
    }

    pub fn brighten(self, value: i32) -> WorkingImage {
        WorkingImage{
            fut: Box::new(self.fut.map(move |info| {
                ImageInfo { image: info.image.brighten(value), ..info }
            })),
        }
    }

    pub fn contrast(self, value: f32) -> WorkingImage {
        WorkingImage{
            fut: Box::new(self.fut.map(move |info| {
                ImageInfo { image: info.image.adjust_contrast(value), ..info }
            })),
        }
    }

    pub fn crop(self, x: u32, y: u32, width: u32, height: u32) -> WorkingImage {
        WorkingImage{
            fut: Box::new(self.fut.map(move |mut info| {
                ImageInfo { image: info.image.crop(x, y, width, height), ..info }
            })),
        }
    }

    pub fn flip(self, horiz: bool) -> WorkingImage {
        WorkingImage{
            fut: Box::new(self.fut.map(move |info| {
                ImageInfo {
                    image: if horiz { info.image.fliph() } else { info.image.flipv() },
                    ..info
                }
            })),
        }
    }

    pub fn grayscale(self) -> WorkingImage {
        WorkingImage{
            fut: Box::new(self.fut.map(move |info| {
                ImageInfo { image: info.image.grayscale(), ..info }
            })),
        }
    }

    pub fn resize(
        self,
        w: f64, w_pct: bool, h: f64, h_pct: bool,
        exact: bool, filter: Option<FilterType>,
    ) -> WorkingImage {
        WorkingImage {
            fut: Box::new(self.fut.map(move |info| {
                let in_filter = filter.unwrap_or(FilterType::Lanczos3).into();
                let width = val_or_pct(info.image.width(), w, w_pct);
                let height = val_or_pct(info.image.height(), h, h_pct);
                ImageInfo {
                    image: if exact {
                        info.image.resize_exact(width, height, in_filter)
                    } else {
                        info.image.resize(width, height, in_filter)
                    },
                    ..info
                }
            })),
        }
    }

    pub fn rotate(self, deg: u32) -> WorkingImage {
        WorkingImage {
            fut: Box::new(self.fut.and_then(move |info| {
                Ok(ImageInfo {
                    image: match deg {
                        90 => info.image.rotate90(),
                        180 => info.image.rotate180(),
                        270 => info.image.rotate270(),
                        _ => return Err(err_str_to_js("Can only rotate 90, 180, or 270".to_string())),
                    },
                    ..info
                })
            })),
        }
    }

    pub fn sharpen(self, sigma: f32, threshold: i32) -> WorkingImage {
        WorkingImage{
            fut: Box::new(self.fut.map(move |info| {
                ImageInfo { image: info.image.unsharpen(sigma, threshold), ..info }
            })),
        }
    }

    pub fn thumbnail(self, w: f64, w_pct: bool, h: f64, h_pct: bool, exact: bool) -> WorkingImage {
        WorkingImage {
            fut: Box::new(self.fut.map(move |info| {
                let width = val_or_pct(info.image.width(), w, w_pct);
                let height = val_or_pct(info.image.height(), h, h_pct);
                ImageInfo {
                    image: if exact {
                        info.image.thumbnail_exact(width, height)
                    } else {
                        info.image.thumbnail(width, height)
                    },
                    ..info
                }
            })),
        }
    }
}

fn rgba_string(color: &str) -> Result<image::Rgba<u8>, String> {
    if color.len() != 6 {
        return Err("Only hex colors starting with hash currently accepted".to_string())
    }
    let r = u8::from_str_radix(&color[..2], 16).map_err(|e| e.to_string())?;
    let g = u8::from_str_radix(&color[2..4], 16).map_err(|e| e.to_string())?;
    let b = u8::from_str_radix(&color[4..], 16).map_err(|e| e.to_string())?;
    Ok(image::Rgba([r, g, b, 0xFF]))
}

fn val_or_pct(orig: u32, v: f64, v_pct: bool) -> u32 {
    (if v_pct { orig as f64 * v } else { v }) as u32
}

#[wasm_bindgen]
#[derive(Copy, Clone)]
pub enum ImageFormat {
    PNG,
    JPEG,
}

// Ref: https://github.com/rustwasm/wasm-bindgen/issues/1496#issuecomment-519255857
#[wasm_bindgen]
pub fn image_format_from_string(s: &str) -> Result<ImageFormat, JsValue> {
    ImageFormat::from_str(s).map_err(|e| JsValue::from(js_sys::Error::new(&e)))
}

impl ImageFormat {
    fn mime_type(&self) -> &str {
        match self {
            ImageFormat::PNG => "image/png",
            ImageFormat::JPEG => "image/jpeg",
        }
    }
}

impl FromStr for ImageFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "PNG" => Ok(ImageFormat::PNG),
            "JPEG" => Ok(ImageFormat::JPEG),
            _ => Err(format!("Unknown format: {}", s)),
        }
    }
}

impl TryFrom<image::ImageFormat> for ImageFormat {
    type Error = String;

    fn try_from(f: image::ImageFormat) -> Result<Self, Self::Error> {
        match f {
            image::ImageFormat::PNG => Ok(ImageFormat::PNG),
            image::ImageFormat::JPEG => Ok(ImageFormat::JPEG),
            _ => Err(format!("Unhandled format: {:?}", f)),
        }
    }
}

impl Into<image::ImageFormat> for ImageFormat {
    fn into(self) -> image::ImageFormat {
         match self {
            ImageFormat::PNG => image::ImageFormat::PNG,
            ImageFormat::JPEG => image::ImageFormat::JPEG,
        }
    }
}

#[wasm_bindgen]
#[derive(Copy, Clone)]
pub enum FilterType {
    Nearest,
    Triangle,
    CatmullRom,
    Gaussian,
    Lanczos3,
}

// Ref: https://github.com/rustwasm/wasm-bindgen/issues/1496#issuecomment-519255857
#[wasm_bindgen]
pub fn filter_type_from_string(s: &str) -> Result<FilterType, JsValue> {
    FilterType::from_str(s).map_err(|e| JsValue::from(js_sys::Error::new(&e)))
}

impl FromStr for FilterType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Nearest" => Ok(FilterType::Nearest),
            "Triangle" => Ok(FilterType::Triangle),
            "CatmullRom" => Ok(FilterType::CatmullRom),
            "Gaussian" => Ok(FilterType::Gaussian),
            "Lanczos3" => Ok(FilterType::Lanczos3),
            _ => Err(format!("Unknown filter type: {}", s)),
        }
    }
}

impl Into<image::FilterType> for FilterType {
    fn into(self) -> image::FilterType {
        match self {
            FilterType::Nearest => image::FilterType::Nearest,
            FilterType::Triangle => image::FilterType::Triangle,
            FilterType::CatmullRom => image::FilterType::CatmullRom,
            FilterType::Gaussian => image::FilterType::Gaussian,
            FilterType::Lanczos3 => image::FilterType::Lanczos3,
        }
    }
}