//! A demonstration of using `sdl2` to provide events and GFX to draw the UI.

#![allow(unused_variables)]

#[cfg(feature="sdl2")] #[macro_use] extern crate conrod;
#[cfg(feature="sdl2")] extern crate sdl2;
#[cfg(feature="gfx_rs")] extern crate gfx;
#[cfg(feature="gfx_rs")] extern crate gfx_core;

#[cfg(feature="sdl2")]
mod support;


fn main() {
    feature::main();
}

#[cfg(all(feature="sdl2",feature="gfx_rs"))]
mod feature {
    extern crate gfx_window_sdl;
    extern crate gfx_device_gl;
    extern crate find_folder;
    extern crate image;

    use std;

    use conrod;
    use gfx;
    use gfx_core;
    use support;
    use sdl2;

    use gfx::Device;


    const WIN_W: u32 = support::WIN_W;
    const WIN_H: u32 = support::WIN_H;
    const CLEAR_COLOR: [f32; 4] = [0.2, 0.2, 0.2, 1.0];

    type DepthFormat = gfx::format::DepthStencil;
    use conrod::backend::gfx::ColorFormat;

    pub fn main() {
        // Initialize sdl2 context
        let sdl_context = sdl2::init().unwrap();
        let video_subsystem = sdl_context.video().unwrap();

        { // setup multisampling and opengl version
            let gl_attr = video_subsystem.gl_attr();
            gl_attr.set_multisample_buffers(1);
            gl_attr.set_multisample_samples(8);
            gl_attr.set_context_version(3u8, 2u8);
            gl_attr.set_context_profile(sdl2::video::GLProfile::Core);
        }

        let mut builder = video_subsystem
            .window("Conrod with GFX and Sdl2", WIN_W, WIN_H);

        builder.position_centered();
        builder.resizable();
        builder.allow_highdpi();

        // Initialize gfx things
        let (window, glcontext, mut device, mut factory, rtv, ds) =
            gfx_window_sdl::init::<ColorFormat, DepthFormat>(builder)
            .expect("gfx_window_sdl::init failed!");

        video_subsystem.gl_set_swap_interval(1); // vsync on

        let mut encoder: gfx::Encoder<_, _> = factory.create_command_buffer().into();

        let dpi_factor = window.drawable_size().0 as f64 / window.size().0 as f64;

        let mut renderer = conrod::backend::gfx::Renderer::new(&mut factory, &rtv, dpi_factor).unwrap();

        // Create Ui and Ids of widgets to instantiate
        let mut ui = conrod::UiBuilder::new([WIN_W as f64, WIN_H as f64]).theme(support::theme()).build();
        let ids = support::Ids::new(ui.widget_id_generator());

        // Load font from file
        let assets = find_folder::Search::KidsThenParents(3, 5).for_folder("assets").unwrap();
        let font_path = assets.join("fonts/NotoSans/NotoSans-Regular.ttf");
        ui.fonts.insert_from_file(font_path).unwrap();

        // Load the Rust logo from our assets folder to use as an example image.
        fn load_rust_logo<T: gfx::format::TextureFormat,R: gfx_core::Resources, F: gfx::Factory<R>>(factory: &mut F) -> (gfx::handle::ShaderResourceView<R, <T as gfx::format::Formatted>::View>,(u32,u32)) {
            use gfx::{format, texture};
            use gfx::memory::Usage;
            let assets = find_folder::Search::ParentsThenKids(3, 3).for_folder("assets").unwrap();
            let path = assets.join("images/rust.png");
            let rgba_image = image::open(&std::path::Path::new(&path)).unwrap().to_rgba();
            let image_dimensions = rgba_image.dimensions();
            let kind = texture::Kind::D2(
                image_dimensions.0 as texture::Size,
                image_dimensions.1 as texture::Size,
                texture::AaMode::Single
            );
            let info = texture::Info {
                kind: kind,
                levels: 1,
                format: <T::Surface as format::SurfaceTyped>::get_surface_type(),
                bind: gfx::SHADER_RESOURCE,
                usage: Usage::Dynamic,
            };
            let raw = factory.create_texture_raw(info, Some(<T::Channel as format::ChannelTyped>::get_channel_type()) , Some(&[rgba_image.into_raw().as_slice()])).unwrap();
            let tex = gfx_core::memory::Typed::new(raw);
            let view = factory.view_texture_as_shader_resource::<T>(
                &tex, (0,0), format::Swizzle::new()
            ).unwrap();
            (view,image_dimensions)
        }

        let mut image_map = conrod::image::Map::new();
        let rust_logo = image_map.insert(load_rust_logo::<conrod::backend::gfx::ColorFormat,_,_>(&mut factory));

        // Demonstration app state that we'll control with our conrod GUI.
        let mut app = support::DemoApp::new(rust_logo);

        let mut events = sdl_context.event_pump().unwrap(); // poll sdl2 events

        'main: loop {
            if let Some(primitives) = ui.draw_if_changed() {
                let (win_w, win_h) = window.drawable_size();
                let dims = (win_w as f32, win_h as f32);
                let dpi_factor = win_w / window.size().0;

                //Clear the window
                renderer.clear(&mut encoder, CLEAR_COLOR);

                renderer.fill(&mut encoder,dims, dpi_factor as f64, primitives, &image_map);

                renderer.draw(&mut factory,&mut encoder,&image_map);

                encoder.flush(&mut device);
                window.gl_swap_window(); // swap buffers
                device.cleanup();
            }

            for event in events.poll_iter() {
                let (win_w, win_h) = {
                    let (w,h) = window.size();
                    (w as f64, h as f64)
                };

                // Convert sdl2 event to conrod event, requires conrod to be built with the `sdl2` feature
                if let (Some(event), mb_extra_event) = conrod::backend::sdl2::convert_event(event.clone(), win_w, win_h) {
                    ui.handle_event(event);
                    if let Some(extra_event) = mb_extra_event {
                        ui.handle_event(extra_event);
                    }
                }

                // Close window if the escape key or the exit button is pressed
                match event {
                    sdl2::event::Event::Quit { .. } => break 'main,
                    sdl2::event::Event::KeyDown { keycode: Some(keycode), .. } => {
                        if keycode == sdl2::keyboard::Keycode::Escape {
                            break 'main
                        }
                    },
                    sdl2::event::Event::Window { win_event : sdl2::event::WindowEvent::Resized(_,_), .. } => {
                        if let Some((new_rtv, _)) = new_views(&window, &rtv, &ds) {
                            renderer.on_resize(new_rtv);
                        }
                    },
                    _ => {},
                }
            }

            // Update widgets if any event has happened
            if ui.global_input().events().next().is_some() {
                let mut ui = ui.set_widgets();
                support::gui(&mut ui, &ids, &mut app);
            }
        }
    }

    /// Update render targets. This is necessary after a window is resized.
    fn new_views<Cf,Df>(window: &sdl2::video::Window,
                           rtv: &gfx_core::handle::RenderTargetView<gfx_device_gl::Resources, Cf>,
                           ds: &gfx_core::handle::DepthStencilView<gfx_device_gl::Resources, Df>)
        -> Option<(gfx_core::handle::RenderTargetView<gfx_device_gl::Resources, Cf>,
                   gfx_core::handle::DepthStencilView<gfx_device_gl::Resources, Df>)>
        where Cf: gfx_core::format::RenderFormat,
              Df: gfx_core::format::DepthFormat,
    {
        use gfx_core::memory::Typed;
        use gfx_core::texture;

        let old_dim = rtv.get_dimensions();
        assert_eq!(old_dim, ds.get_dimensions());

        let dim = {
            let (w,h) = window.drawable_size();
            let aa = window.subsystem().gl_attr().multisample_samples() as texture::NumSamples;
            (w as texture::Size, h as texture::Size, 1, aa.into())
        };

        if old_dim != dim {
            let (raw_rtv, raw_ds) =
                gfx_device_gl::create_main_targets_raw(dim, Cf::get_format().0, Df::get_format().0);
            Some((Typed::new(raw_rtv), Typed::new(raw_ds)))
        } else {
            None
        }
    }
}

#[cfg(not(all(feature="sdl2",feature="gfx_rs")))]
mod feature {
    pub fn main() {
        println!("This example requires the `sdl2` feature and the `gfx_rs` feature. \
                 Try running `cargo run --release --no-default-features --features=\"sdl2 gfx_rs\" --example <example_name>`");
   }
}
