// The boilerplate code is shamelessly stolen from the imgui rust examples.
// If you get a linking error, download SDL2.lib from the SDL website.

use glow::HasContext;
use imgui::{Context, WindowFlags};
use imgui_glow_renderer::AutoRenderer;
use imgui_sdl2_support::SdlPlatform;
use sdl2::{
    event::Event,
    video::{GLProfile, Window},
};

struct GameState {
    chess_board: chess::ChessBoard,
    chess_representation: [(i8, i8); 64],
    moving_piece: usize,
    is_white_turn: bool,
    is_promoting: bool,
}

impl GameState {
    fn new_game() -> GameState {
        let chess_board = chess::ChessBoard::new();
        let chess_representation = chess_board.get_board();

        GameState {
            chess_board,
            chess_representation,
            moving_piece: 65,
            is_white_turn: true,
            is_promoting: false,
        }
    }
    fn do_move(self: &mut Self, from: usize, to: usize) {
        if self.chess_board.game_end {
            println!("The game is over, moving is not allowed");
            return
        }
        if self.is_promoting {
            println!("Please promote first");
        }
        self.chess_board.move_by_index(from, to);
        self.chess_representation = self.chess_board.get_board();
        self.is_white_turn = !self.is_white_turn;
        self.is_promoting = self.chess_board.can_promote()
    }
    fn promote(self: &mut Self, piece: i8) {
        self.chess_board.promote(piece);
        self.is_promoting = false;
        self.chess_representation = self.chess_board.get_board();
    }
}

fn piece_to_unicode(piece: i8) -> &'static str {
    // We force imgui to give our buttons an ID of 0 (which is then
    // hashed with the square number later.
    match piece {
        0 => "###0",
        1 => "\u{265F}###0",
        2 => "\u{265C}###0",
        3 => "\u{265E}###0",
        4 => "\u{265D}###0",
        5 => "\u{265B}###0",
        6 => "\u{265A}###0",
        _ => "###0",
    }
}

fn team_to_color(team: i8) -> [f32; 4] {
    if team == 1 { [1.0, 0.0, 1.0, 1.0 ] } else { [1.0, 1.0, 0.0, 1.0] }
}

fn draw_chess(ui: &imgui::Ui, game_state: &mut GameState) {
    let display_size = ui.io().display_size;
    let cell_size = (display_size[0].min(display_size[1]) / 8.).round() - 10.;
    let draw_list = ui.get_window_draw_list();
    draw_list.add_rect([0., 0.], [cell_size * 8., cell_size * 8.], 0xFF664488).filled(true).build();
    for i in 0..64 {
        let row = i >> 3;
        let column = (i & 7) as f32;
        let begin = [cell_size * column, cell_size * (row as f32)];
        let end = [cell_size * (column + 1.), cell_size * ((row + 1) as f32)];
        if (i+row) % 2 == 1 {
            draw_list.add_rect(begin, end, 0xFF336622).filled(true).build();
        }
    }

    let _no_padding = ui.push_style_var(imgui::StyleVar::ItemSpacing([0., 0.]));
    let _no_bg = ui.push_style_color(imgui::StyleColor::Button, [0., 0., 0., 0.]);
    let _no_border_popup = ui.push_style_var(imgui::StyleVar::PopupBorderSize(0.));
    let _no_bg = ui.push_style_color(imgui::StyleColor::PopupBg, [0., 0., 0., 0.]);
    for i in 0..64 {
        if i % 8 != 0 {
            ui.same_line();
        }
        let _id = ui.push_id_usize(i);
        let (piece, team) = game_state.chess_representation[i];
        let piece_unicode = if game_state.moving_piece == i {
            "###0"
        } else {
            piece_to_unicode(piece)
        };
        let fg_color = team_to_color(team);
        let _color_stck = ui.push_style_color(imgui::StyleColor::Text, fg_color);
        ui.button_with_size(piece_unicode, [cell_size, cell_size]);

        if let Some(_) = ui.drag_drop_source_config("move").begin_payload(i) {
            game_state.moving_piece = i;
        }
        if let Some(v) = ui.drag_drop_target() {
            if let Some(p) = v.accept_payload("move", imgui::DragDropFlags::empty()) {
                if let Ok(v) = p {
                    let source: usize = v.data;
                    game_state.do_move(source, i);
                }
            }
        }
    }
    if game_state.moving_piece < 65 {
        let (piece, team) = game_state.chess_representation[game_state.moving_piece];
        if piece != 0 {
            let fg_color = team_to_color(team);
            let _color_stck = ui.push_style_color(imgui::StyleColor::Text, fg_color);
            let mouse_pos = ui.io().mouse_pos;
            draw_list.add_text([mouse_pos[0] - 16., mouse_pos[1] - 16.], fg_color, &piece_to_unicode(piece)[0..3]);
        }
        if !ui.is_mouse_dragging(imgui::MouseButton::Left) {
            game_state.moving_piece = 65;
        }
    }
    if game_state.chess_board.game_end {
        draw_list.add_text([cell_size * 4. - 150., cell_size * 4.], 0xFFFFFFFF, "IT'S SO OVER!");
        if ui.button("Restart") {
            *game_state = GameState::new_game();
        }
    } else {
        let t = if game_state.is_white_turn {
            "It is white's turn"
        } else {
            "It is black's turn"
        };
        ui.text(t);
    }

    if game_state.is_promoting {
        let window = ui.window("Promotion")
            .size([230., 0.], imgui::Condition::Always)
            .flags(WindowFlags::NO_COLLAPSE);
        if let Some(_t) = window.begin() {
            if ui.button("\u{265C}") {
                game_state.promote(2);
            }
            ui.same_line();
            if ui.button("\u{265E}") {
                game_state.promote(3);
            }
            ui.same_line();
            if ui.button("\u{265D}") {
                game_state.promote(4);
            }
            ui.same_line();
            if ui.button("\u{265B}") {
                game_state.promote(5);
            }
        }
    }
}

fn draw_ui(ui: &imgui::Ui, game_state: &mut GameState) {
    let window = ui.window("Chess")
        .flags(WindowFlags::NO_DECORATION | WindowFlags::NO_BACKGROUND)
        .position([0., 0.], imgui::Condition::Always)
        .size(ui.io().display_size, imgui::Condition::Always);
    if let Some(_t) = window.begin() {
        draw_chess(ui, game_state);
    }
}

fn glow_context(window: &Window) -> glow::Context {
    unsafe {
        glow::Context::from_loader_function(|s| window.subsystem().gl_get_proc_address(s) as _)
    }
}

fn main() {
    let sdl = sdl2::init().unwrap();
    let video_subsystem = sdl.video().unwrap();

    let gl_attr = video_subsystem.gl_attr();

    gl_attr.set_context_version(3, 3);
    gl_attr.set_context_profile(GLProfile::Core);

    let window = video_subsystem
        .window("GChess: Chess 4 real Gs", 720, 720)
        .allow_highdpi()
        .opengl()
        .position_centered()
        .resizable()
        .build()
        .unwrap();

    let gl_context = window.gl_create_context().unwrap();
    window.gl_make_current(&gl_context).unwrap();

    window.subsystem().gl_set_swap_interval(1).unwrap();

    let gl = glow_context(&window);

    let mut imgui = Context::create();

    imgui.set_ini_filename(None);
    imgui.set_log_filename(None);

    let font = imgui.fonts().add_font(&[imgui::FontSource::TtfData {
        data: include_bytes!("DejaVuSans.ttf"),
        size_pixels: 48.,
        config: Some(imgui::FontConfig {
            glyph_ranges: imgui::FontGlyphRanges::from_slice(&[
                1,      // Ascii
                128,
                0x265A, // Chess pieces in Unicode
                0x265F,
                0,

            ]),
            ..Default::default()
        }),
    }]);

    imgui.io_mut().config_flags |= imgui::ConfigFlags::NO_MOUSE_CURSOR_CHANGE;

    let mut platform = SdlPlatform::init(&mut imgui);
    let mut renderer = AutoRenderer::initialize(gl, &mut imgui).unwrap();

    let mut event_pump = sdl.event_pump().unwrap();

    let mut game_state = GameState::new_game();

    'main: loop {
        for event in event_pump.poll_iter() {
            platform.handle_event(&mut imgui, &event);

            if let Event::Quit { .. } = event {
                break 'main;
            }
        }

        platform.prepare_frame(&mut imgui, &window, &event_pump);

        let ui = imgui.new_frame();
        {
            let _font = ui.push_font(font);
            let _no_padding = ui.push_style_var(imgui::StyleVar::WindowPadding([0., 0.]));
            draw_ui(ui, &mut game_state);
        }


        let draw_data = imgui.render();

        unsafe { renderer.gl_context().clear(glow::COLOR_BUFFER_BIT) };

        renderer.render(draw_data).unwrap();

        window.gl_swap_window();
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}
