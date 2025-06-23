use crate::app_state::State;
use std::sync::Arc;
use winit::{
    application::ApplicationHandler,
    event::*,
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowId},
};

#[cfg(target_os = "macos")]
use winit::platform::macos::WindowAttributesExtMacOS;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::{prelude::*, JsCast};

struct App {
    state: Option<State>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_none() {
            let mut window_attributes = Window::default_attributes()
                .with_title("wcanvas");
            
            #[cfg(target_os = "macos")]
            {
                window_attributes = window_attributes
                    .with_titlebar_transparent(true)
                    .with_title_hidden(false)
                    .with_fullsize_content_view(true);
            }
            
            let window = event_loop.create_window(window_attributes).unwrap();

            #[cfg(target_arch = "wasm32")]
            {
                use winit::dpi::PhysicalSize;
                use winit::platform::web::WindowExtWebSys;
                
                let canvas = web_sys::Element::from(window.canvas().unwrap());
                let canvas_html: web_sys::HtmlCanvasElement = canvas.clone().dyn_into().unwrap();
                
                web_sys::window()
                    .and_then(|win| win.document())
                    .and_then(|doc| {
                        let dst = doc.get_element_by_id("wasm-example")?;
                        dst.append_child(&canvas).ok()?;
                        Some(())
                    })
                    .expect("Couldn't append canvas to document body.");

                canvas_html.style().set_property("width", "100vw").unwrap();
                canvas_html.style().set_property("height", "100vh").unwrap();
                canvas_html.style().set_property("display", "block").unwrap();
                
                let update_canvas_size = || {
                    let web_window = web_sys::window().unwrap();
                    let device_pixel_ratio = web_window.device_pixel_ratio();
                    let css_width = web_window.inner_width().unwrap().as_f64().unwrap();
                    let css_height = web_window.inner_height().unwrap().as_f64().unwrap();
                    
                    let width = (css_width * device_pixel_ratio) as u32;
                    let height = (css_height * device_pixel_ratio) as u32;
                    
                    canvas_html.set_width(width);
                    canvas_html.set_height(height);
                    
                    canvas_html.style().set_property("width", &format!("{}px", css_width as u32)).unwrap();
                    canvas_html.style().set_property("height", &format!("{}px", css_height as u32)).unwrap();
                    
                    log::info!("Canvas size updated to: {}x{} (CSS: {}x{}, DPR: {})", 
                              width, height, css_width as u32, css_height as u32, device_pixel_ratio);
                    (width, height)
                };
                
                let (width, height) = update_canvas_size();
                let _ = window.request_inner_size(PhysicalSize::new(width, height));
                
                let canvas_clone = canvas_html.clone();
                let resize_closure = Closure::wrap(Box::new(move || {
                    let web_window = web_sys::window().unwrap();
                    let device_pixel_ratio = web_window.device_pixel_ratio();
                    let css_width = web_window.inner_width().unwrap().as_f64().unwrap();
                    let css_height = web_window.inner_height().unwrap().as_f64().unwrap();
                    
                    let width = (css_width * device_pixel_ratio) as u32;
                    let height = (css_height * device_pixel_ratio) as u32;
                    
                    canvas_clone.set_width(width);
                    canvas_clone.set_height(height);
                    canvas_clone.style().set_property("width", &format!("{}px", css_width as u32)).unwrap();
                    canvas_clone.style().set_property("height", &format!("{}px", css_height as u32)).unwrap();
                    
                    log::info!("Window resized to: {}x{} (CSS: {}x{}, DPR: {})", 
                              width, height, css_width as u32, css_height as u32, device_pixel_ratio);
                }) as Box<dyn FnMut()>);
                
                web_sys::window()
                    .unwrap()
                    .add_event_listener_with_callback("resize", resize_closure.as_ref().unchecked_ref())
                    .unwrap();
                
                resize_closure.forget();
            }

            pollster::block_on(async {
                self.state = Some(State::new(Arc::new(window)).await);
            });
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, window_id: WindowId, event: WindowEvent) {
        if let Some(state) = &mut self.state {
            if window_id == state.window().id() {
                if !state.input(&event) {
                    match event {
                        WindowEvent::CloseRequested
                        | WindowEvent::KeyboardInput {
                            event:
                                KeyEvent {
                                    state: ElementState::Pressed,
                                    physical_key: PhysicalKey::Code(KeyCode::Escape),
                                    ..
                                },
                            ..
                        } => event_loop.exit(),
                        WindowEvent::Resized(physical_size) => {
                            log::info!("WindowEvent::Resized: {}x{}", physical_size.width, physical_size.height);
                            state.resize(physical_size);
                        }
                        WindowEvent::RedrawRequested => {
                            state.update();
                            match state.render() {
                                Ok(_) => {}
                                Err(wgpu::SurfaceError::Lost) => state.resize(state.size),
                                Err(wgpu::SurfaceError::OutOfMemory) => event_loop.exit(),
                                Err(e) => eprintln!("{:?}", e),
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(state) = &self.state {
            state.window().request_redraw();
        }
    }
}

pub async fn run() {
    cfg_if::cfg_if! {
        if #[cfg(target_arch = "wasm32")] {
            std::panic::set_hook(Box::new(console_error_panic_hook::hook));
            console_log::init_with_level(log::Level::Info).expect("Couldn't initialize logger");
        } else {
            env_logger::init();
        }
    }

    let event_loop = EventLoop::new().unwrap();
    let mut app = App { state: None };

    event_loop.run_app(&mut app).unwrap();
}
