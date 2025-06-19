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

// Re-export the main public interface
pub use app::run;
pub use vertex::Vertex;

// Re-export for WASM compatibility
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(start))]
pub async fn start() {
    run().await;
}
