extern crate gl;
extern crate sdl2;
extern crate imgui;
extern crate cgmath;
extern crate appbase;
extern crate webutil;
extern crate tetris;
extern crate rand;

extern crate serde;
extern crate base64;
extern crate bincode;
#[macro_use] extern crate serde_derive;

use imgui::*;
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::keyboard::Mod;
use appbase::webrunner;
use std::collections::HashMap;

mod renderer;
mod util;
mod client;

const ZFAR: f32 = 700.0;

enum State {
    MainMenu,
    PreGame,
    Game {
        game: tetris::game::Game,
        paused: bool,
        finished: bool,
    },
    Replay {
        replayer: tetris::replay::Replayer,
    },
    Highscores {
        selected: Option<usize>,
        sort_by_score: bool,
        global: bool,
    }
}

struct TetrisApp {
    windowsize: (f32, f32),
    fixedsize: (f32, f32),
    ui_center: (f32, f32),
    ui_scale: f32,

    ui: Option<State>,
    config: tetris::Config,
    playername: String,

    renderer: renderer::Renderer,

    rotl: bool,
    rotr: bool,

    server: client::ServerConfig,
    requests: Vec<client::Request>,
    load_idtag: appbase::localstorage::StorageLoad,
    load_name: appbase::localstorage::StorageLoad,

    scores_global: Vec<tetris::PlayedGame>,
    scores_local: Vec<tetris::PlayedGame>,
    last_global: Vec<tetris::PlayedGame>,
    last_local: Vec<tetris::PlayedGame>,

    replays: HashMap<usize, tetris::replay::Replay>
}

impl TetrisApp {
    fn save(&mut self, game: &tetris::game::Game) {
        self.requests.push(self.server.upload_replay(&self.playername, game.replay()));
    }

    fn window<'ui,'p>(&self, ui: &'ui imgui::Ui, name: &'p ImStr, pos: (f32, f32), size: (f32, f32), font_scale: f32) -> Window<'ui, 'p> {
        ui.window(name)
            .position((self.ui_center.0 + pos.0 * self.ui_scale, self.ui_center.1 + pos.1 * self.ui_scale), ImGuiCond::Always)
            .size((size.0 * self.ui_scale, size.1 * self.ui_scale), ImGuiCond::Always)
            .title_bar(false)
            .movable(false)
            .resizable(false)
            .collapsible(false)
            .font_scale(font_scale * self.ui_scale)
    }

    fn process_finished_requests(&mut self) {
        let mut answers = Vec::new();

        self.requests.retain(|request| {
            match request.response() {
                client::Response::Waiting => true,
                client::Response::HttpError(err) => {
                    println!("HttpError: {}", err);
                    false
                }
                client::Response::ParseError(err) => {
                    println!("ParseError: {}", err);
                    false
                }
                client::Response::Success(msg) => {
                    answers.push(msg);
                    false
                }
            }
        });

        for msg in answers {
            match msg {
                tetris::networking::ServerAnswer::InvalidMessage(err) => println!("{:?}", err),
                tetris::networking::ServerAnswer::ServerError(err) => println!("{:?}", err),
                tetris::networking::ServerAnswer::HighscoreList { by_score, idtagged, from, to, data } => {
                    let mut dst = if idtagged {
                        if by_score { &mut self.scores_local } else { &mut self.last_local }
                    } else {
                        if by_score { &mut self.scores_global } else { &mut self.last_global }
                    };
                    dst.clone_from(&data);
                },
                tetris::networking::ServerAnswer::ReplayList { data } => {
                    for r in data {
                        self.replays.insert(r.0, r.1);
                    }
                }
                tetris::networking::ServerAnswer::UploadResult(result) => {
                    self.request_highscores();
                }
            };
        }
    }

    fn request_highscores(&mut self) {
        self.requests.push(self.server.request_scores(false, false));
        self.requests.push(self.server.request_scores(false, true));
        self.requests.push(self.server.request_scores(true, false));
        self.requests.push(self.server.request_scores(true, true));
    }

    fn check_idtag(&mut self) {
        if let Some(new_idtag) = self.load_idtag.consume(|data| {
            let mut idtag = String::from_utf8(data).unwrap_or(client::gen_idtag());
            if idtag.len() != 32 {
                idtag = client::gen_idtag();
            }
            idtag
        }, || {
            client::gen_idtag()
        }) {
            appbase::localstorage::store("TETRIS", "idtag", &new_idtag.clone().into_bytes());
            self.server.set_idtag(&new_idtag);
            self.request_highscores();
        }
    }

    fn check_player_name(&mut self) {
        if let Some(name) = self.load_name.consume(|data| { String::from_utf8(data).ok() }, || { None }) {
            if let Some(name) = name {
                self.playername = name;
            }
        }
    }
}

impl webrunner::WebApp for TetrisApp {
    fn new(windowsize: (u32, u32)) -> Self {
        let mut ret = TetrisApp {
            windowsize: (windowsize.0 as f32, windowsize.1 as f32),
            fixedsize: (710.0, 650.0),
            ui_scale: 1.0,
            ui_center: (0.5 * windowsize.0 as f32, 0.5 * windowsize.1 as f32),
            ui: Some(State::MainMenu),
            config: tetris::Config::new(),
            playername: String::from("Your name please?"),
            renderer: renderer::Renderer::new(
                renderer::Rectangle::new(-150.0, -300.0, 300.0, 600.0),
                renderer::Rectangle::new(220.0, -300.0, 120.0, 120.0),
                renderer::Rectangle::new(220.0, -120.0, 180.0, 200.0),
                renderer::Rectangle::new(-400.0, -300.0, 300.0, 400.0),
                ZFAR
            ),
            rotl: false,
            rotr: false,
            server: client::ServerConfig::new(),
            requests: Vec::new(),
            load_idtag: appbase::localstorage::load("TETRIS", "idtag"),
            load_name: appbase::localstorage::load("TETRIS", "playername"),
            scores_global: Vec::new(),
            scores_local: Vec::new(),
            last_global: Vec::new(),
            last_local: Vec::new(),
            replays: HashMap::new(),
        };

        ret.request_highscores();

        ret
    }

    fn resize(&mut self, size: (u32, u32)) {
        self.windowsize = (size.0 as f32, size.1 as f32);
        let sx = self.windowsize.0 / self.fixedsize.0;
        let sy = self.windowsize.1 / self.fixedsize.1;
        self.ui_scale = sx.min(sy);
        self.ui_center = (0.5 * size.0 as f32, 0.5 * size.1 as f32);
    }

    fn render(&mut self, dt: f32) {
        unsafe {
            gl::ClearColor(0.0, 0.15, 0.2, 1.0);
            gl::Clear(gl::COLOR_BUFFER_BIT | gl::DEPTH_BUFFER_BIT);
        }

        self.renderer.clear();

        // go through server responses
        self.process_finished_requests();
        self.check_idtag();

        // Advance running game?
        let mut bg = false;
        self.ui = Some(match self.ui.take().unwrap() {
            State::Game{mut game, paused, mut finished} => {
                if !finished && !paused {
                    if let tetris::game::Outcome::Death = game.frame(dt) {
                        self.save(&game);
                        finished = true;
                    }
                }

                self.renderer.set_state(game.timestamp(), game.snapshot());
                State::Game{game, paused, finished}
            },
            State::Replay{mut replayer} => {
                let adv = dt * replayer.speed;
                replayer.advance(adv);
                self.renderer.set_state(replayer.timestamp(), replayer.snapshot());
                State::Replay{replayer}
            },
            State::Highscores{selected, sort_by_score, global} => { bg = true; State::Highscores{selected, sort_by_score, global} },
            State::MainMenu => { bg = true; State::MainMenu },
            State::PreGame => { bg = true; State::PreGame },
        });

        // Update Burn animation
        let nearfac = 0.1;
        let far = ZFAR;
        let proj = cgmath::frustum(
            -0.5 * nearfac * self.windowsize.0,
            0.5 * nearfac * self.windowsize.0,
            -0.5 * nearfac * self.windowsize.1,
            0.5 * nearfac * self.windowsize.1,
            nearfac * far,
            2.0 * far);
        let proj = proj * cgmath::Matrix4::from_nonuniform_scale(self.ui_scale, -self.ui_scale, 1.0);
        if bg {
            self.renderer.render_background(dt, &proj);
        } else {
            self.renderer.render(&proj);
        }
    }

    fn do_ui(&mut self, ui: &imgui::Ui, keymod: sdl2::keyboard::Mod) {
        let mb1x = -350.0;
        let mb2x = 150.0;
        let mby = -60.0;
        let mbw = 200.0;
        let mbh = 120.0;

        self.ui = Some(match self.ui.take().unwrap() {
            State::MainMenu => {
                let mut ret = State::MainMenu;
                self.window(ui, im_str!("mainmenu_start"), (mb1x, mby), (mbw, mbh), 2.0).build(|| {
                    ui.set_cursor_pos((20.0 * self.ui_scale, 20.0 * self.ui_scale));
                    if ui.button(im_str!("Start Game"), ((mbw - 40.0)* self.ui_scale, (mbh - 40.0) * self.ui_scale)) {
                        self.check_player_name();
                        ret = State::PreGame;
                    }
                });
                self.window(ui, im_str!("mainmenu_highscores"), (mb2x, mby), (mbw, mbh), 2.0).build(|| {
                    ui.set_cursor_pos((20.0 * self.ui_scale, 20.0 * self.ui_scale));
                    if ui.button(im_str!("Highscores"), ((mbw - 40.0) * self.ui_scale, (mbh - 40.0) * self.ui_scale)) {
                        ret = State::Highscores{selected: None, sort_by_score: true, global: true};
                    }
                });
                ret
            },

            State::Highscores{mut selected, mut sort_by_score, mut global} => {
                let mut ret = None;

                self.window(ui, im_str!("highscores_back"), (mb2x, mby), (mbw, mbh), 2.0).build(|| {
                    ui.set_cursor_pos((20.0 * self.ui_scale, 20.0 * self.ui_scale));
                    if ui.button(im_str!("Back"), ((mbw - 40.0)* self.ui_scale, (mbh - 40.0) * self.ui_scale)) {
                        ret = Some(State::MainMenu);
                    }
                });

                self.window(ui, im_str!("highscores_list"), (mb1x, mby - 150.0), (mb2x - mb1x, mbh + 300.0), 1.2).build(|| {
                    // get high-score list
                    let scores = if global {
                        if sort_by_score { &self.scores_global } else { &self.last_global }
                    } else {
                        if sort_by_score { &self.scores_local } else { &self.last_local }
                    };

                    ui.set_cursor_pos((20.0 * self.ui_scale, 20.0 * self.ui_scale));
                    let btn = if sort_by_score { "By Score##highscores" } else { "By Date##highscores" };
                    if ui.button(im_str!("{}", btn), (100.0 * self.ui_scale, 30.0 * self.ui_scale)) {
                        sort_by_score = !sort_by_score;
                    }

                    if let Some(idx) = selected {
                        if idx < scores.len() {
                            ui.same_line(0.5 * (mb2x - mb1x - 100.0) * self.ui_scale);
                            let replay_id = scores[idx].replay();
                            let replay = self.replays.get(&replay_id);
                            let btn = if replay.is_some() { "Replay##highscores" } else { "Downloading...##nighscores" };
                            if ui.button(im_str!("{}", btn), (100.0 * self.ui_scale, 30.0 * self.ui_scale)) {
                                if let Some(replay) = replay {
                                    ret = Some(State::Replay {
                                        replayer: tetris::replay::Replayer::new(replay)
                                    });
                                }
                            }
                        }
                    }

                    ui.same_line((mb2x - mb1x - 120.0) * self.ui_scale);
                    let btn = if global { "Global##highscores" } else { "Local##highscores" };
                    if ui.button(im_str!("{}", btn), (100.0 * self.ui_scale, 30.0 * self.ui_scale)) {
                        global = !global;
                    }

                    ui.columns(5, im_str!("High-Scores List"), true);
                    ui.separator();

                    ui.text("Score"); ui.next_column();
                    ui.text("Name"); ui.next_column();
                    ui.text("Lv Start"); ui.next_column();
                    ui.text("Lv End"); ui.next_column();
                    ui.text("Date"); ui.next_column();
                    ui.separator();

                    for score in scores.iter().enumerate() {
                        if ui.selectable(im_str!("{}##entry{}", score.1.score().to_string(), score.0),
                                         selected.map_or(false, |s| s == score.0),
                                         imgui::ImGuiSelectableFlags::SpanAllColumns,
                                         (0.0, 0.0)) {
                            selected = Some(score.0);

                            // start to download replay, if it isn't available yet
                            let replay = self.replays.get(&score.1.replay());
                            if replay.is_none() {
                                self.requests.push(self.server.request_replay(score.1.replay()));
                            }
                        }
                        ui.next_column();
                        ui.text(score.1.name().to_string()); ui.next_column();
                        ui.text(score.1.start_level().to_string()); ui.next_column();
                        ui.text(score.1.end_level().to_string()); ui.next_column();
                        ui.text(score.1.time_str()); ui.next_column();
                        ui.separator();
                    }
                });

                ret.unwrap_or(State::Highscores{selected, sort_by_score, global})
            }
            State::PreGame => {
                let mut ret = None;

                self.window(ui, im_str!("pregame_start"), (mb1x, mby), (mbw, mbh), 2.0).build(|| {
                    ui.set_cursor_pos((20.0 * self.ui_scale, 20.0 * self.ui_scale));
                    if ui.button(im_str!("Start"), ((mbw - 40.0)* self.ui_scale, (mbh - 40.0) * self.ui_scale)) {
                        self.renderer.gen_new_colors();
                        appbase::localstorage::store("TETRIS", "playername", self.playername.as_bytes());
                        ret = Some(State::Game {
                            game: tetris::game::Game::new(&self.config),
                            paused: false,
                            finished: false
                        });
                    }
                });

                self.window(ui, im_str!("pregame_back"), (mb2x, mby), (mbw, mbh), 2.0).build(|| {
                    ui.set_cursor_pos((20.0 * self.ui_scale, 20.0 * self.ui_scale));
                    if ui.button(im_str!("Back"), ((mbw - 40.0)* self.ui_scale, (mbh - 40.0) * self.ui_scale)) {
                        ret = Some(State::MainMenu);
                    }
                });

                self.window(ui, im_str!("pregame_options"), (mb1x + mbw, mby - 100.0), (mb2x - mb1x - mbw, 300.0), 1.5).build(|| {
                    ui.set_cursor_pos((20.0 * self.ui_scale, 20.0 * self.ui_scale));
                    ui.text("Starting Level");

                    ui.set_cursor_pos((20.0 * self.ui_scale, 45.0 * self.ui_scale));
                    ui.push_item_width(mb2x - mb1x - mbw);
                    ui.slider_int(im_str!("##pregamestartlevel"), &mut self.config.level, 0, 20)
                        .build();

                    ui.set_cursor_pos((20.0 * self.ui_scale, 100.0 * self.ui_scale));
                    ui.text("Player Name");

                    ui.set_cursor_pos((20.0 * self.ui_scale, 125.0 * self.ui_scale));
                    ui.push_item_width(mb2x - mb1x - mbw);
                    let mut pname = ImString::with_capacity(1024);
                    pname.push_str(&self.playername);
                    ui.input_text(im_str!("##pregame_playername"), &mut pname).build();
                    self.playername = pname.to_str().to_string();

                    ui.set_cursor_pos((20.0 * self.ui_scale, 180.0 * self.ui_scale));
                    ui.checkbox(im_str!("Ghost Piece"), &mut self.renderer.ghost_piece);

                    ui.set_cursor_pos((20.0 * self.ui_scale, 230.0 * self.ui_scale));
                    ui.checkbox(im_str!("3D Pieces"), &mut self.renderer.threed);
                });

                ret.unwrap_or(State::PreGame)
            }
            State::Game{game, paused, mut finished} => {
                self.renderer.do_ui(ui, self.ui_center, self.ui_scale);
                let mut ret = None;

                ui.with_color_var(imgui::ImGuiCol::WindowBg, (0.0, 0.0, 0.0, 0.0), || {
                    self.window(ui, im_str!("Playing Game UI#window"), (200.0, 150.0), (200.0, 200.0), 1.5).build(|| {
                        if !finished {
                            if ui.button(im_str!("Give up!##playing"), (140.0 * self.ui_scale, 40.0 * self.ui_scale)) && !paused {
                                self.save(&game);
                                finished = true;
                            }
                        } else {
                            if ui.button(im_str!("Back##fromgametomain"), (140.0 * self.ui_scale, 40.0 * self.ui_scale)) {
                                ret = Some(State::MainMenu)
                            }
                        }

                        ui.new_line();

                        if finished && ui.button(im_str!("Watch Replay##aftergamefinished"), (140.0 * self.ui_scale, 40.0 * self.ui_scale)) {
                            ret = Some(State::Replay {
                                replayer: tetris::replay::Replayer::new(game.replay())
                            });
                        }
                    });
                });

                if paused {
                    self.window(ui, im_str!("pausedplayinggame"), (-100.0, -50.0), (200.0, 100.0), 2.5).build(|| {
                        ui.new_line(); ui.text("  Paused")
                    });
                }

                ret.unwrap_or(State::Game{game, paused, finished})
            }
            State::Replay{mut replayer} => {
                self.renderer.do_ui(ui, self.ui_center, self.ui_scale);
                let mut ret = None;

                self.window(ui, im_str!("Replayer UI#window"), (200.0, 100.0), (200.0, 200.0), 1.2).build(|| {
                    ui.text("Speed");
                    ui.slider_float(im_str!("##replayspeedslider"), &mut replayer.speed, -20.0, 20.0)
                        .power(2.0)
                        .build();

                    ui.new_line();

                    ui.text("Time");
                    let mut curr = replayer.frame();
                    ui.slider_float(im_str!("##replaytimeslider"), &mut curr, 0.0, replayer.length())
                        .build();
                    replayer.jump(curr);

                    ui.new_line();

                    if ui.button(im_str!("Back##tomainmenu"), (140.0 * self.ui_scale, 40.0 * self.ui_scale)) {
                        ret = Some(State::MainMenu);
                    }
                });

                if replayer.paused {
                    self.window(ui, im_str!("pausedplayinggame"), (-100.0, -50.0), (200.0, 100.0), 2.5).build(|| {
                        ui.new_line(); ui.text("  Paused")
                    });
                }

                ret.unwrap_or(State::Replay{replayer})
            },
        });
    }

    fn event(&mut self, event: &Event) {
        match event {
            Event::KeyDown{keycode, keymod, .. } => {
                let ctrl = keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD);

                fn adjust_speed(value: &mut f32, delta: f32) {
                    let v = (value.signum() * value.abs().sqrt() + delta);
                    *value = v.signum() * v.powi(2).min(20.0);
                }

                match &mut self.ui.as_mut().unwrap() {
                    State::Game{ref mut game, ref mut paused, ..} => match keycode.unwrap() {
                        Keycode::Left => game.left(true),
                        Keycode::Right => game.right(true),
                        Keycode::Down => game.down(true),
                        Keycode::Z => if !self.rotl {
                            self.rotl = true;
                            game.rotate(false);
                        },
                        Keycode::X => if !self.rotr {
                            self.rotr = true;
                            game.rotate(true);
                        },
                        Keycode::Return => *paused = !*paused,
                        _ => {}
                    }
                    State::Replay { ref mut replayer } => match keycode.unwrap() {
                        Keycode::Left => replayer.advance(if ctrl { -10.0 } else { -1.0 }),
                        Keycode::Right => replayer.advance(if ctrl { 10.0 } else { 1.0 }),
                        Keycode::Up => adjust_speed(&mut replayer.speed, 0.1),
                        Keycode::Down => adjust_speed(&mut replayer.speed, -0.1),
                        Keycode::Return => replayer.paused = !replayer.paused,
                        _ => {}
                    }
                    _ => {}
                }
            },
            Event::KeyUp{keycode, .. } => {
                match &mut self.ui.as_mut().unwrap() {
                    State::Game { ref mut game, .. } => match keycode.unwrap() {
                        Keycode::Left => game.left(false),
                        Keycode::Right => game.right(false),
                        Keycode::Down => game.down(false),
                        Keycode::Z => self.rotl = false,
                        Keycode::X => self.rotr = false,
                        _ => {}
                    },
                    _ => {}
                }
            },
            _ => {}
        }
    }
}

fn main() {
    webrunner::AppRunner::<TetrisApp>::start("Tetris!");
}
