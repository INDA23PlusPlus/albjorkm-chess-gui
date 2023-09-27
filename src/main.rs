use glow::HasContext;
use imgui::Context;
use imgui_glow_renderer::AutoRenderer;
use imgui_sdl2_support::SdlPlatform;
use sdl2::{
    event::Event,
    video::{GLProfile, Window},
};

// If you get a linking error, download SDL2.lib from the SDL website.

// Create a new glow context.
fn glow_context(window: &Window) -> glow::Context {
    unsafe {
        glow::Context::from_loader_function(|s| window.subsystem().gl_get_proc_address(s) as _)
    }
}

fn draw_chess(ui: &imgui::Ui) {
    /*if let Some(_) = ui.begin_table_with_sizing("Chess", 8, imgui::TableFlags::empty(), [720., 720.], 720.) {
        // we have to call next_row because we didn't make headers..
        ui.table_next_column();
        for i in 0..64 {
            let _id = ui.push_id_usize(i);
            ui.button_with_size("Btn", [32., 32.0]);
            
            if let Some(v) = ui.drag_drop_source_config("DRAG").begin_payload("Payload") {
                ui.text("Sending text");
                v.end()
            }
            if let Some(v) = ui.drag_drop_target() {
                if let Some(p) = v.accept_payload("DRAG", imgui::DragDropFlags::empty()) {
                    if let Ok(v) = p {
                        let s: &str = v.data;
                        println!("drop: {s}");
                    }
                }
            }
            ui.table_next_column();
        }
    }*/

    // we have to call next_row because we didn't make headers..
    let display_size = ui.io().display_size;
    let cell_size = display_size[0].min(display_size[1]) / 8. - 10.;
    for i in 0..64 {
        if i % 8 != 0 {
            ui.same_line();
        }
        let _id = ui.push_id_usize(i);
        ui.button_with_size("Btn", [cell_size, cell_size]);

        if let Some(v) = ui.drag_drop_source_config("DRAG").begin_payload("Payload") {
            ui.text("Sending text");
            v.end()
        }
        if let Some(v) = ui.drag_drop_target() {
            if let Some(p) = v.accept_payload("DRAG", imgui::DragDropFlags::empty()) {
                if let Ok(v) = p {
                    let s: &str = v.data;
                    println!("drop: {s}");
                }
            }
        }
    }
}

fn draw_ui(ui: &imgui::Ui) {
    
    let window_token = ui.window("Chess")
        .no_decoration()
        .position([0., 0.], imgui::Condition::Always)
        .size(ui.io().display_size, imgui::Condition::Always).begin();
    if let Some(_t) = window_token {
        draw_chess(ui);
    }
    /*for _ in 0..8 {
        for _ in 0..8 {
            ui.button_with_size("Button", [32f32, 32f32]);
        }
        ui.new_line();
    }*/
}

fn main() {
    /* initialize SDL and its video subsystem */
    let sdl = sdl2::init().unwrap();
    let video_subsystem = sdl.video().unwrap();

    /* hint SDL to initialize an OpenGL 3.3 core profile context */
    let gl_attr = video_subsystem.gl_attr();

    gl_attr.set_context_version(3, 3);
    gl_attr.set_context_profile(GLProfile::Core);

    /* create a new window, be sure to call opengl method on the builder when using glow! */
    let window = video_subsystem
        .window("GChess: Chess 4 real Gs", 720, 720)
        .allow_highdpi()
        .opengl()
        .position_centered()
        .resizable()
        .build()
        .unwrap();

    /* create a new OpenGL context and make it current */
    let gl_context = window.gl_create_context().unwrap();
    window.gl_make_current(&gl_context).unwrap();

    /* enable vsync to cap framerate */
    window.subsystem().gl_set_swap_interval(1).unwrap();

    /* create new glow and imgui contexts */
    let gl = glow_context(&window);

    /* create context */
    let mut imgui = Context::create();

    /* disable creation of files on disc */
    imgui.set_ini_filename(None);
    imgui.set_log_filename(None);

    /* setup platform and renderer, and fonts to imgui */
    imgui
        .fonts()
        .add_font(&[imgui::FontSource::DefaultFontData { config: None }]);

    /* create platform and renderer */
    let mut platform = SdlPlatform::init(&mut imgui);
    let mut renderer = AutoRenderer::initialize(gl, &mut imgui).unwrap();

    /* start main loop */
    let mut event_pump = sdl.event_pump().unwrap();

    'main: loop {
        for event in event_pump.poll_iter() {
            /* pass all events to imgui platfrom */
            platform.handle_event(&mut imgui, &event);

            if let Event::Quit { .. } = event {
                break 'main;
            }
        }

        /* call prepare_frame before calling imgui.new_frame() */
        platform.prepare_frame(&mut imgui, &window, &event_pump);

        let ui = imgui.new_frame();
        //ui.show_demo_window(&mut true);
        draw_ui(ui);

        /* render */
        let draw_data = imgui.render();

        unsafe { renderer.gl_context().clear(glow::COLOR_BUFFER_BIT) };
        renderer.render(draw_data).unwrap();

        window.gl_swap_window();
    }
}
