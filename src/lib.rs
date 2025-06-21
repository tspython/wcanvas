mod app;
mod app_state;
mod canvas;
mod drawing;
mod event_handler;
mod renderer;
mod state;
mod text_renderer;
mod texture;
mod ui;
mod update_logic;
mod vertex;

pub use app::run;
pub use vertex::Vertex;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub async fn start() {
    run().await;
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn start() {
    run().await;
}
