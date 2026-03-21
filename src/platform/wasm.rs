use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;

pub fn save_to_local_storage(key: &str, json: &str) {
    if let Some(window) = web_sys::window() {
        if let Ok(Some(storage)) = window.local_storage() {
            if let Err(e) = storage.set_item(key, json) {
                log::warn!("Failed to save to localStorage: {:?}", e);
            }
        }
    }
}

pub fn load_from_local_storage(key: &str) -> Option<String> {
    let window = web_sys::window()?;
    let storage = window.local_storage().ok()??;
    storage.get_item(key).ok()?
}

pub fn trigger_download(filename: &str, content: &str) {
    let window = match web_sys::window() {
        Some(w) => w,
        None => return,
    };
    let document = match window.document() {
        Some(d) => d,
        None => return,
    };

    // Create a Blob from the JSON content
    let parts = js_sys::Array::new();
    parts.push(&JsValue::from_str(content));

    let mut options = web_sys::BlobPropertyBag::new();
    options.set_type("application/json");

    let blob = match web_sys::Blob::new_with_str_sequence_and_options(&parts, &options) {
        Ok(b) => b,
        Err(e) => {
            log::warn!("Failed to create blob: {:?}", e);
            return;
        }
    };

    let url = match web_sys::Url::create_object_url_with_blob(&blob) {
        Ok(u) => u,
        Err(e) => {
            log::warn!("Failed to create object URL: {:?}", e);
            return;
        }
    };

    // Create a hidden anchor element, set href and download, click it
    let anchor: web_sys::HtmlAnchorElement = match document
        .create_element("a")
        .ok()
        .and_then(|e| e.dyn_into::<web_sys::HtmlAnchorElement>().ok())
    {
        Some(a) => a,
        None => return,
    };

    anchor.set_href(&url);
    anchor.set_download(filename);
    anchor.style().set_property("display", "none").ok();

    if let Some(body) = document.body() {
        let _ = body.append_child(&anchor);
        anchor.click();
        let _ = body.remove_child(&anchor);
    }

    let _ = web_sys::Url::revoke_object_url(&url);
}

pub fn trigger_file_open(callback: impl FnOnce(String) + 'static) {
    let window = match web_sys::window() {
        Some(w) => w,
        None => return,
    };
    let document = match window.document() {
        Some(d) => d,
        None => return,
    };

    let input: web_sys::HtmlInputElement = match document
        .create_element("input")
        .ok()
        .and_then(|e| e.dyn_into::<web_sys::HtmlInputElement>().ok())
    {
        Some(i) => i,
        None => return,
    };

    input.set_type("file");
    input.set_accept(".wcanvas,.json");
    input.style().set_property("display", "none").ok();

    let callback = std::cell::RefCell::new(Some(callback));
    let input_clone = input.clone();

    let closure = Closure::once(Box::new(move || {
        if let Some(files) = input_clone.files() {
            if let Some(file) = files.get(0) {
                let reader = web_sys::FileReader::new().unwrap();
                let reader_clone = reader.clone();

                let onload = Closure::once(Box::new(move || {
                    if let Ok(result) = reader_clone.result() {
                        if let Some(text) = result.as_string() {
                            if let Some(cb) = callback.borrow_mut().take() {
                                cb(text);
                            }
                        }
                    }
                }));

                reader.set_onload(Some(onload.as_ref().unchecked_ref()));
                onload.forget();

                let _ = reader.read_as_text(&file);
            }
        }
    }) as Box<dyn FnOnce()>);

    input.set_onchange(Some(closure.as_ref().unchecked_ref()));
    closure.forget();

    if let Some(body) = document.body() {
        let _ = body.append_child(&input);
        input.click();
        let _ = body.remove_child(&input);
    }
}
