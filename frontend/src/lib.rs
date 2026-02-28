mod api;
mod mapping;
mod models;
mod state;
mod views;

use leptos::*;
use wasm_bindgen::prelude::*;
use views::app::App;

#[wasm_bindgen(start)]
pub fn main() {
    console_error_panic_hook::set_once();
    _ = console_log::init_with_level(log::Level::Debug);

    let document = web_sys::window()
        .and_then(|window| window.document())
        .expect("document should be available");
    let root = document
        .get_element_by_id("root")
        .expect("root element should exist")
        .dyn_into::<web_sys::HtmlElement>()
        .expect("root should be an HtmlElement");

    mount_to(root, || view! { <App/> })
}
