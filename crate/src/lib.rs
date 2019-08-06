
use futures::Future;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::{future_to_promise,JsFuture};
use std::io::Cursor;

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

pub fn set_panic_hook() {
    // When the `console_error_panic_hook` feature is enabled, we can call the
    // `set_panic_hook` function at least once during initialization, and then
    // we will get better error messages if our code ever panics.
    //
    // For more details see
    // https://github.com/rustwasm/console_error_panic_hook#readme
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
}

#[wasm_bindgen]
pub fn init() {
    set_panic_hook();
}

#[wasm_bindgen]
pub fn run(url: &str) -> String {
    web_sys::console::log_2(&"Some URL".into(), &url.into());
    format!("You came from URL: {}", url)
}

macro_rules! console_log {
    ($($t:tt)*) => (web_sys::console::log_1(&format!($($t)*).into()))
}

#[wasm_bindgen]
pub fn rotate(url: &str) -> js_sys::Promise {
    console_log!("Loading from URL: {}", url);
    let global = js_sys::global().unchecked_into::<web_sys::WorkerGlobalScope>();
    let resp_promise = global.fetch_with_str(url);
    let future = JsFuture::from(resp_promise).and_then(|resp_val| {
        assert!(resp_val.is_instance_of::<web_sys::Response>());
        let resp: web_sys::Response = resp_val.dyn_into().unwrap();
        let buf_promise = resp.array_buffer().unwrap();
        JsFuture::from(buf_promise).map(move |buf_val| {
            assert!(buf_val.is_instance_of::<js_sys::ArrayBuffer>());
            let uint8_arr: js_sys::Uint8Array = js_sys::Uint8Array::new(&buf_val);
            console_log!("Response size: {}", uint8_arr.length());
            let mut bytes = vec![0; uint8_arr.length() as usize];
            uint8_arr.copy_to(&mut bytes);
            let mut buf = Cursor::new(Vec::new());
            console_log!("Loading from memory");
            let img = image::load_from_memory(&bytes).unwrap();
            console_log!("Rotating from memory");
            let rotated = img.rotate90();
            let headers = web_sys::Headers::new().unwrap();
            // headers.set("Content-Type", "image/png").unwrap();
            // console_log!("Writing as PNG");
            // rotated.write_to(&mut buf, image::ImageFormat::PNG).unwrap();
            headers.set("Content-Type", "image/jpeg").unwrap();
            console_log!("Writing as JPEG");
            rotated.write_to(&mut buf, image::ImageFormat::JPEG).unwrap();
            console_log!("Sending new response size: {}", buf.get_ref().len());
            let new_arr = js_sys::Uint8Array::from(buf.get_ref().as_slice());
            let new_resp = web_sys::Response::new_with_opt_buffer_source_and_init(Some(&new_arr), web_sys::ResponseInit::new().headers(&headers)).unwrap();
            JsValue::from(new_resp)
        })
    });
    future_to_promise(future)
}