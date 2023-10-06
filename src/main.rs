// The boilerplate code is shamelessly stolen from the imgui rust examples.
// If you get a linking error, download SDL2.lib from the SDL website.

use std::{io::Read, marker::PhantomData};

use chess_network_protocol::{ClientToServerHandshake,
                             ClientToServer, ServerToClient,
                             ServerToClientHandshake,
                             Piece, Move, Joever};
use chess_network_protocol::Color::White;
use glow::HasContext;
use imgui::{Context, WindowFlags};
use imgui_glow_renderer::AutoRenderer;
use imgui_sdl2_support::SdlPlatform;
use sdl2::{
    event::Event,
    video::{GLProfile, Window},
};

fn chess_piece_to_wire(square: (i8, i8)) -> Piece {
    match square {
        (1, -1) => Piece::WhitePawn,
        (2, -1) => Piece::WhiteRook,
        (3, -1) => Piece::WhiteKnight,
        (4, -1) => Piece::WhiteBishop,
        (5, -1) => Piece::WhiteQueen,
        (6, -1) => Piece::WhiteKing,
        (1, 1) => Piece::BlackPawn,
        (2, 1) => Piece::BlackRook,
        (3, 1) => Piece::BlackKnight,
        (4, 1) => Piece::BlackBishop,
        (5, 1) => Piece::BlackQueen,
        (6, 1) => Piece::BlackKing,
        _ => Piece::None,
    }
}

fn chess_representaiton_to_wire(data: &[(i8, i8); 64]) -> [[Piece; 8]; 8] {
    let mut result = [[Piece::None; 8]; 8];
    for i in 0..64 {
        let square = data[i];
        let row = i >> 3;
        let column = i & 7;
        result[7 - row][column] = chess_piece_to_wire(square);
    }
    result
}

fn wire_to_chess_representation(data: &[[Piece; 8]; 8]) -> [(i8, i8); 64] {
    let mut result = [(0, 0); 64];
    for i in 0..64 {
        let row = i >> 3;
        let column = i & 7;
        let square = data[7 - row][column];
        result[i] = match square {
            Piece::WhitePawn   => (1, -1),
            Piece::WhiteRook   => (2, -1),
            Piece::WhiteKnight => (3, -1),
            Piece::WhiteBishop => (4, -1),
            Piece::WhiteQueen  => (5, -1),
            Piece::WhiteKing   => (6, -1),
            Piece::BlackPawn   => (1, 1),
            Piece::BlackRook   => (2, 1),
            Piece::BlackKnight => (3, 1),
            Piece::BlackBishop => (4, 1),
            Piece::BlackQueen  => (5, 1),
            Piece::BlackKing   => (6, 1),
            _ => (0, 0),
        }
    }
    result
}

enum GameMode {
    Undecided(String),
    HostWaitForOpponent(std::net::TcpListener),
    Host(std::net::TcpStream, JsonPoller<ClientToServerHandshake,
                                         ClientToServer>),
    Client(std::net::TcpStream, JsonPoller<ServerToClientHandshake,
                                           ServerToClient>),
    Local,
}

enum UnsentNetMove {
    None,
    PendingPromotion(Move),
    Unsent(Move),
}

struct ChessState {
    chess_board: chess::ChessBoard,
    chess_representation: [(i8, i8); 64],
    moving_piece: usize,
    is_white_turn: bool,
    is_promoting: bool,
    is_game_over: bool,
    is_client: bool,
    unsent_net_move: UnsentNetMove,
}

impl ChessState {
    fn do_move(self: &mut Self, from: usize, to: usize) -> bool {
        if self.is_game_over {
            println!("The game is over, moving is not allowed");
            return false
        }
        if self.is_promoting {
            println!("Please promote first");
        }

        let to_rank = to >> 3;

        let net_move = Move {
            end_y: 7 - to_rank,
            end_x: to & 7,
            start_y: 7 - (from >> 3),
            start_x: from & 7,
            promotion: Piece::None,
        };

        let mut did_move = false;

        if self.is_client {
            self.is_promoting = self.chess_representation[from].0 == 1 &&
                (to_rank == 0 || to_rank == 7);
            did_move = true;
        } else if self.chess_board.move_by_index(from, to) {
            self.chess_representation = self.chess_board.get_board();
            self.is_white_turn = !self.is_white_turn;
            self.is_promoting = self.chess_board.can_promote();


            self.unsent_net_move = if self.is_promoting {
                // If we are promoting, we don't want to send the
                // update just yet.
                UnsentNetMove::PendingPromotion(net_move)
            } else {
                UnsentNetMove::Unsent(net_move)
            };

            self.is_game_over = self.chess_board.is_game_ended();
            did_move = true;
        }

        if did_move {
            self.unsent_net_move = if self.is_promoting {
                // If we are promoting, we don't want to send the
                // update just yet.
                UnsentNetMove::PendingPromotion(net_move)
            } else {
                UnsentNetMove::Unsent(net_move)
            };
        }

        did_move
    }
    fn promote(self: &mut Self, piece: i8) {
        self.chess_board.promote(piece);
        self.is_promoting = false;
        if !self.is_client {
            self.chess_representation = self.chess_board.get_board();
            self.is_game_over = self.chess_board.is_game_ended();
        }
        let UnsentNetMove::PendingPromotion(mut mv)
            = self.unsent_net_move else {
            panic!("promote() called with bad unsent_net_move value");
        };
        let color = if self.is_white_turn { 1 } else { -1 };
        mv.promotion = chess_piece_to_wire((piece, color));
        self.unsent_net_move = UnsentNetMove::Unsent(mv);
    }
    fn ingest_client_move(self: &mut Self, mv: &Move)
        -> bool {
        let from = (7 - mv.start_y) << 3 | mv.start_x;
        let to = (7 - mv.end_y) << 3 | mv.end_x;
        let result = self.do_move(from, to);
        if self.is_promoting {
            let piece = match mv.promotion {
                Piece::WhitePawn   | Piece::BlackPawn   => 1,
                Piece::WhiteRook   | Piece::BlackRook   => 2,
                Piece::WhiteKnight | Piece::BlackKnight => 3,
                Piece::WhiteBishop | Piece::BlackBishop => 4,
                Piece::WhiteQueen  | Piece::BlackQueen  => 5,
                Piece::WhiteKing   | Piece::BlackKing   => 6,
                _ => 0,
            };
            self.promote(piece);
        }
        result
    }
    fn to_joever(&self) -> Joever {
        if self.is_game_over {
            Joever::Indeterminate
        } else {
            Joever::Ongoing
        }
    }
}

/// All the state related to the running of the game including netcode.
struct GameState {
    chess_state: ChessState,
    mode: GameMode,
    host_is_white: bool,
}

impl GameState {
    fn new_game() -> GameState {
        let chess_board = chess::ChessBoard::new();
        let chess_representation = chess_board.get_board();

        GameState {
            chess_state: ChessState {
                chess_board,
                chess_representation,
                moving_piece: 65,
                unsent_net_move: UnsentNetMove::None,
                is_white_turn: true,
                is_promoting: false,
                is_game_over: false,
                is_client: false,
            },
            mode: GameMode::Undecided(String::from("localhost")),
            host_is_white: true,
        }
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

fn draw_chess(ui: &imgui::Ui, chess_state: &mut ChessState, can_move: bool) {
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
        let (piece, team) = chess_state.chess_representation[i];
        let piece_unicode = if chess_state.moving_piece == i {
            "###0"
        } else {
            piece_to_unicode(piece)
        };
        let fg_color = team_to_color(team);
        let _color_stck = ui.push_style_color(imgui::StyleColor::Text, fg_color);
        ui.button_with_size(piece_unicode, [cell_size, cell_size]);

        if !can_move {
            continue
        }

        if let Some(_) = ui.drag_drop_source_config("move").begin_payload(i) {
            chess_state.moving_piece = i;
        }
        if let Some(v) = ui.drag_drop_target() {
            if let Some(p) = v.accept_payload("move", imgui::DragDropFlags::empty()) {
                if let Ok(v) = p {
                    let source: usize = v.data;
                    chess_state.do_move(source, i);
                }
            }
        }
    }
    if chess_state.moving_piece < 65 {
        let (piece, team) = chess_state.chess_representation[chess_state.moving_piece];
        if piece != 0 {
            let fg_color = team_to_color(team);
            let _color_stck = ui.push_style_color(imgui::StyleColor::Text, fg_color);
            let mouse_pos = ui.io().mouse_pos;
            draw_list.add_text([mouse_pos[0] - 16., mouse_pos[1] - 16.], fg_color, &piece_to_unicode(piece)[0..3]);
        }
        if !ui.is_mouse_dragging(imgui::MouseButton::Left) {
            chess_state.moving_piece = 65;
        }
    }
    if chess_state.is_game_over {
        draw_list.add_text([cell_size * 4. - 150., cell_size * 4.], 0xFFFFFFFF, "IT'S SO OVER!");
    } else {
        let t = if chess_state.is_white_turn {
            "It is white's turn"
        } else {
            "It is black's turn"
        };
        ui.text(t);
    }

    if chess_state.is_promoting {
        let window = ui.window("Promotion")
            .size([230., 0.], imgui::Condition::Always)
            .flags(WindowFlags::NO_COLLAPSE);
        if let Some(_t) = window.begin() {
            if ui.button("\u{265C}") {
                chess_state.promote(2);
            }
            ui.same_line();
            if ui.button("\u{265E}") {
                chess_state.promote(3);
            }
            ui.same_line();
            if ui.button("\u{265D}") {
                chess_state.promote(4);
            }
            ui.same_line();
            if ui.button("\u{265B}") {
                chess_state.promote(5);
            }
        }
    }
}

fn draw_ui(ui: &imgui::Ui, game_state: &mut GameState) {
    if let GameMode::Undecided(address) = &mut game_state.mode {
        let window = ui.window("Select Mode")
            .size([500., 0.], imgui::Condition::Once);
        if let Some(_t) = window.begin() {
            if ui.button("Local Play") {
                game_state.mode = GameMode::Local;
                return
            }
            if ui.button("Host Game") {
                let listener = std::net::TcpListener::bind("127.0.0.1:8483")
                    .unwrap();
                listener.set_nonblocking(true).unwrap();
                game_state.mode = GameMode::HostWaitForOpponent(listener);
                return
            }
            let _ = ui.input_text("Address", address).build();
            if ui.button("Join Game") {
                let address = if address.contains(":") {
                    address.clone()
                } else {
                    format!("{address}:8483")
                };
                println!("[client] attempting to connect to: {address}");
                let stream = std::net::TcpStream::connect(address).unwrap();
                stream.set_nonblocking(true).unwrap();

                let handshake = ClientToServerHandshake {
                    server_color: chess_network_protocol::Color::Black,
                };
                serde_json::to_writer(&stream, &handshake).unwrap();


                game_state.mode = GameMode::Client(stream, JsonPoller::new());
                return
            }
        }
        return
    }
    if let GameMode::HostWaitForOpponent(_) = &mut game_state.mode {
        let window = ui.window("Awaiting opponent!")
            .size([500., 0.], imgui::Condition::Once);
        if let Some(_t) = window.begin() {
            ui.text("Please wait");
        }
        return
    }

    let window = ui.window("Chess")
        .flags(WindowFlags::NO_DECORATION | WindowFlags::NO_BACKGROUND)
        .position([0., 0.], imgui::Condition::Always)
        .size(ui.io().display_size, imgui::Condition::Always);
    if let Some(_t) = window.begin() {
        // Moves can be made if it is the turn of the host. Or we are not
        // running as a host.
        let is_whites_turn = game_state.chess_state.is_white_turn;
        let can_move = match game_state.mode {
            GameMode::Host(..) => game_state.host_is_white == is_whites_turn,
            _ => true
        };

        draw_chess(ui, &mut game_state.chess_state, can_move);

        if game_state.chess_state.is_game_over {
            if ui.button("Restart") {
                *game_state = GameState::new_game();
            }
        }
    }
}

fn glow_context(window: &Window) -> glow::Context {
    unsafe {
        glow::Context::from_loader_function(|s|
            window.subsystem().gl_get_proc_address(s) as _)
    }
}

enum JsonState {
    Normal,
    String,
    StringEscape,
}

/// Because serde_json exposes no way to know when serialization ended we
/// implement this ourselves.
struct JsonFinder {
    pub length: usize,
    nesting: i32,
    state: JsonState,
}

impl JsonFinder {
    fn new() -> Self {
        return JsonFinder {
            length: 0,
            nesting: 0,
            state: JsonState::Normal,
        }
    }
    fn reset(&mut self) {
        self.length = 0;
        self.nesting = 0;
        self.state = JsonState::Normal;
    }

    /// Returns true once the finder has found a complete JSON object.
    fn feed(&mut self, bytes: &[u8]) -> bool {
        for b in bytes {
            match self.state {
                JsonState::Normal => match b {
                    b'"' => self.state = JsonState::String,
                    b'{' => self.nesting += 1,
                    b'}' => {
                        self.nesting -= 1;
                        if self.nesting <= 0 {
                            return true;
                        }
                    },
                    _ => {},
                },
                JsonState::StringEscape => self.state = JsonState::String,
                JsonState::String => match b {
                    b'"'  => self.state = JsonState::Normal,
                    b'\\' => self.state = JsonState::StringEscape,
                    _ => {}
                }
            }
            self.length += 1;
        }
        false
    }
}

struct JsonPoller<Handshake: serde::de::DeserializeOwned,
                  Data: serde::de::DeserializeOwned> {
    finder: JsonFinder,
    buf: Vec<u8>,
    handshake_complete: bool,

    // This is done such that the compiler doesn't complain about unused
    // generics
    _phantom1: std::marker::PhantomData<*const Handshake>,
    _phantom2: std::marker::PhantomData<*const Data>,

}

#[derive(Eq, PartialEq, Debug)]
enum Packet<Handshake, Data> {
    Handshake(Handshake),
    Data(Data),
}

impl<Handshake: serde::de::DeserializeOwned,
     Data: serde::de::DeserializeOwned> JsonPoller<Handshake, Data> {
    fn new() -> Self {
        return JsonPoller::<Handshake, Data> {
            finder: JsonFinder::new(),
            buf: vec![],
            handshake_complete: false,
            _phantom1: PhantomData,
            _phantom2: PhantomData,
        }
    }
    fn feed(&mut self, data: &[u8], into: &mut Vec<Packet<Handshake, Data>>) {
        let start = self.buf.len();
        self.buf.extend_from_slice(data);
        //println!("feed: {:#?}", std::str::from_utf8(&self.buf));
        let mut scan_slice = start .. self.buf.len();
        //println!("scan: {:#?}", scan_slice);
        while self.finder.feed(&self.buf[scan_slice.clone()]) {
            let data = &self.buf[0..self.finder.length + 1];
            //println!("data: {:#?}", std::str::from_utf8(data));
            if self.handshake_complete {
                match serde_json::from_slice(data) {
                    Ok(v) => into.push(Packet::Data(v)),
                    Err(e) => eprintln!("data parse error: {e}"),
                }
            } else {
                self.handshake_complete = true;
                match serde_json::from_slice(data) {
                    Ok(v) => into.push(Packet::Handshake(v)),
                    Err(e) => eprintln!("handshake parse error: {e}"),
                }
            };
            let new_length = self.buf.len() - self.finder.length - 1;
            //println!("new_length: {new_length}");
            let slice = self.finder.length + 1 .. self.buf.len();
            self.buf.copy_within(slice, 0);
            self.buf.truncate(new_length);
            self.finder.reset();
            scan_slice = 0..new_length;
            //println!("feed 2: {:#?}", std::str::from_utf8(&self.buf));
        }
    }
}

fn send_server_handshake(stream: &mut std::net::TcpStream,
                         chess_state: &ChessState) {
    let handshake = ServerToClientHandshake {
        features: vec![
            chess_network_protocol::Features::EnPassant,
            chess_network_protocol::Features::Castling,
            chess_network_protocol::Features::Promotion
        ],
        board: chess_representaiton_to_wire(&chess_state.chess_representation),
        moves: vec![],
        joever: chess_state.to_joever(),
    };
    serde_json::to_writer(stream, &handshake).unwrap();
}

fn handle_client_move(stream: &mut std::net::TcpStream,
                      mv: &Move,
                      chess_state: &mut ChessState) {
    assert!(!chess_state.is_client);
    let good = chess_state.ingest_client_move(mv);
    let board =
        chess_representaiton_to_wire(&chess_state.chess_representation);
    if !good {
        // If the move fails, we immedietly tell the client.
        let state = ServerToClient::Error {
            board,
            moves: vec![],
            joever: chess_state.to_joever(),
            message: "Bad move!".into()
        };
        serde_json::to_writer(stream, &state).unwrap();
    }
}

fn send_client_move(stream: &mut std::net::TcpStream,
                    chess_state: &mut ChessState) {
    let UnsentNetMove::Unsent(mv) = chess_state.unsent_net_move else {
        return
    };

    // If an unsent move has been created before the handshake marks the
    // chess state as client state - something has gone terribly wrong.
    assert!(chess_state.is_client);

    chess_state.unsent_net_move = UnsentNetMove::None;
    let send_move = ClientToServer::Move(mv);
    serde_json::to_writer(stream, &send_move).unwrap();
}

fn synchronize_board_state(stream: &mut std::net::TcpStream,
                           chess_state: &mut ChessState) {
    let UnsentNetMove::Unsent(move_made) = chess_state.unsent_net_move else {
        // No need to perform any work.
        return
    };
    println!("attempt synch");
    chess_state.unsent_net_move = UnsentNetMove::None;

    let board =
        chess_representaiton_to_wire(&chess_state.chess_representation);
    let state = ServerToClient::State {
        board,
        moves: vec![],
        joever: chess_state.to_joever(),
        move_made,
    };
    serde_json::to_writer(stream, &state).unwrap();
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

    let mut buffer = [0u8; 65535];

    let mut from_server_packets = vec![];
    let mut from_client_packets = vec![];


    'main: loop {
        for event in event_pump.poll_iter() {
            platform.handle_event(&mut imgui, &event);

            if let Event::Quit { .. } = event {
                break 'main;
            }
        }

        match &mut game_state.mode {
            GameMode::HostWaitForOpponent(listener) => {
                if let Ok((stream, _)) = listener.accept() {
                    stream.set_nonblocking(true).unwrap();
                    game_state.mode
                        = GameMode::Host(stream, JsonPoller::new());
                }
            }
            GameMode::Host(stream, poller) => {
                let buffer_read = stream.read(&mut buffer).unwrap_or_default();
                poller.feed(&buffer[0..buffer_read], &mut from_client_packets);
                for packet in &from_client_packets {
                    println!("[server] packet received {packet:#?}");
                    match packet {
                        Packet::Handshake(h) => {
                            let is_white = h.server_color == White;
                            game_state.host_is_white = is_white;
                            let state = &game_state.chess_state;
                            send_server_handshake(stream, state);
                        }
                        Packet::Data(d) => {
                            match d {
                                ClientToServer::Move(m) => {
                                    let white_turn = game_state
                                        .chess_state
                                        .is_white_turn;
                                    if game_state.host_is_white == white_turn {
                                        // It isn't the client's turn yet!
                                        continue
                                    }
                                    let s = &mut game_state.chess_state;
                                    handle_client_move(stream, m, s);
                                },
                                ClientToServer::Resign | ClientToServer::Draw
                                    => {
                                    println!("Such dishonorable actions,");
                                    println!("are not supported!");
                                },

                            }
                        }
                    }
                }
                from_client_packets.clear();
                synchronize_board_state(stream, &mut game_state.chess_state);
            }
            GameMode::Client(stream, poller) => {
                let buffer_read = stream.read(&mut buffer).unwrap_or_default();
                poller.feed(&buffer[0..buffer_read], &mut from_server_packets);
                //println!("read: {buffer_read}");
                for packet in &from_server_packets {
                    println!("[client] packet received {packet:#?}");
                    match packet {
                        Packet::Handshake(h) => {
                            game_state.chess_state.is_client = true;
                            game_state.chess_state.chess_representation =
                                wire_to_chess_representation(&h.board);
                        }
                        Packet::Data(d) => {
                            match d {
                                ServerToClient::State
                                    { board, joever, .. }
                                | ServerToClient::Resigned
                                    { board, joever, .. }
                                | ServerToClient::Error
                                    { board, joever, .. } => {
                                    game_state
                                        .chess_state
                                        .chess_representation =
                                        wire_to_chess_representation(&board);
                                    let over = joever != &Joever::Ongoing;
                                    game_state.chess_state.is_game_over = over;
                                    game_state.chess_state.is_white_turn =
                                        !game_state.chess_state.is_white_turn;
                                }
                                ServerToClient::Draw { board, .. } => {
                                    game_state
                                        .chess_state
                                        .chess_representation =
                                        wire_to_chess_representation(&board);
                                    game_state.chess_state.is_game_over = true;
                                }
                            };
                        }
                    }
                }
                from_server_packets.clear();
                send_client_move(stream, &mut game_state.chess_state);
            }
            GameMode::Undecided(_) | GameMode::Local => {}
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

#[cfg(test)]
mod tests {
    use crate::{JsonFinder, JsonPoller, Packet};
    use serde::Deserialize;

    #[test]
    pub fn json_finder() {
        let mut finder = JsonFinder::new();
        assert_eq!(finder.feed(b"{\"hi\": 3"), false);
        assert_eq!(finder.feed(b"2 } excess data here"), true);
        assert_eq!(finder.length, 10);

        finder.reset();
        assert_eq!(finder.feed(b"{\"hi\": \""), false);
        assert_eq!(finder.feed(b"s\\\\t\\\"r\" } excess data here"), true);
        assert_eq!(finder.length, 17);
    }

    #[derive(Deserialize, Debug, Eq, PartialEq)]
    struct TestStruct {
        hi: String,
    }

    #[test]
    pub fn json_poller() {
        let mut poller  = JsonPoller::<TestStruct, TestStruct>::new();
        let mut into = vec![];
        poller.feed(b"{\"hi\": \"", &mut into);
        assert_eq!(into, vec![]);
        poller.feed(b"s\\\\t\\\"r\" }{\"hi\": \"hey\"}", &mut into);
        assert_eq!(into, vec![
            Packet::Handshake(TestStruct  {
                hi: "s\\t\"r".into(),
            }),
            Packet::Data(TestStruct {
                hi: "hey".into(),
            })
        ]);
        into.clear();
        poller.feed(b" ", &mut into);
        assert_eq!(into, vec![]);
    }

    #[test]
    pub fn json_poller_empty() {
        let mut poller  = JsonPoller::<TestStruct, TestStruct>::new();
        let mut into = vec![];
        poller.feed(b"{\"hi\": \"there\"}", &mut into);
        assert_eq!(into, vec![Packet::Handshake(TestStruct  {
                hi: "there".into(),
        })]);
        into.clear();
        poller.feed(b"", &mut into);
        assert_eq!(into, vec![]);
    }
}
