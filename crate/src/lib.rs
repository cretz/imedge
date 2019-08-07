
use futures::Future;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::{future_to_promise,JsFuture};
use std::io::Cursor;
use std::convert::TryFrom;
use std::str::FromStr;
use image::GenericImageView;

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

    #[wasm_bindgen]
    pub fn resize(
        self,
        w: f64, w_pct: bool, h: f64, h_pct: bool,
        exact: bool, filter: Option<FilterType>,
    ) -> WorkingImage {
        WorkingImage {
            fut: Box::new(self.fut.map(move |info| {
                let in_filter = filter.unwrap_or(FilterType::Lanczos3).into();
                let width = if w_pct { info.image.width() as f64 * w } else { w } as u32;
                let height = if h_pct { info.image.height() as f64 * h } else { h } as u32;
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

    #[wasm_bindgen]
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
}

#[wasm_bindgen]
#[derive(Copy, Clone)]
pub enum ImageFormat {
    PNG,
    JPEG,
    GIF,
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
            ImageFormat::GIF => "image/gif",
        }
    }
}

impl FromStr for ImageFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "PNG" => Ok(ImageFormat::PNG),
            "JPEG" => Ok(ImageFormat::JPEG),
            "GIF" => Ok(ImageFormat::GIF),
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
            image::ImageFormat::GIF => Ok(ImageFormat::GIF),
            _ => Err(format!("Unhandled format: {:?}", f)),
        }
    }
}

impl Into<image::ImageFormat> for ImageFormat {
    fn into(self) -> image::ImageFormat {
         match self {
            ImageFormat::PNG => image::ImageFormat::PNG,
            ImageFormat::JPEG => image::ImageFormat::JPEG,
            ImageFormat::GIF => image::ImageFormat::GIF,
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