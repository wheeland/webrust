extern crate gl;
extern crate sdl2;
extern crate imgui;
extern crate cgmath;
extern crate appbase;
extern crate util3d;
extern crate webutil;
#[cfg(target_os = "emscripten")] extern crate emscripten_util;
extern crate tetris;
extern crate rand;
extern crate tinygl;

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
mod client;

const ZFAR: f32 = 700.0;
const FRAME: f32 = 1.0 / 60.0;

#[derive(Clone, Serialize, Deserialize)]
struct PlayerOptions {
    name: String,
    left: i32,
    right: i32,
    drop: i32,
    rotl: i32,
    rotr: i32,
    pause: i32,
    level: i32,
    ghost: bool,
    sound: bool,
    render3d: bool,
}

enum State {
    MainMenu,
    PreGame {
        keyconfig: Option<i32>,
    },
    Game {
        game: tetris::game::Game,
        dtime: f32,
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
    },
    About,
}

struct TetrisApp {
    windowsize: (f32, f32),
    fixedsize: (f32, f32),
    ui_center: (f32, f32),
    ui_scale: f32,

    ui: Option<State>,
    config: tetris::Config,

    player: PlayerOptions,

    renderer: renderer::Renderer,

    rotl: bool,
    rotr: bool,

    server: client::ServerConfig,
    requests: Vec<client::Request>,

    // a unique ID tag will be stored in the browsers local storage, and will be attached to
    // the replays when they get uploaded
    #[cfg(target_os = "emscripten")] load_idtag: emscripten_util::localstorage::StorageLoad,

    // player settings, cached in the local storage
    #[cfg(target_os = "emscripten")] load_player: emscripten_util::localstorage::StorageLoad,

    scores_global: Vec<tetris::PlayedGame>,
    scores_local: Vec<tetris::PlayedGame>,
    last_global: Vec<tetris::PlayedGame>,
    last_local: Vec<tetris::PlayedGame>,

    replays: HashMap<usize, tetris::replay::Replay>,

    fpswidget: appbase::fpswidget::FpsWidget,
}

fn play_sound(tagname: &str) {
    #[cfg(target_os = "emscripten")] {
        emscripten_util::run_javascript(&(String::from("{
            let element = document.getElementById('") + tagname + "');
            element.currentTime = 0;
            element.play();
        }"));
    }
}

impl TetrisApp {
    fn save(&mut self, game: &tetris::game::Game) {
        self.requests.push(self.server.upload_replay(&self.player.name, game.replay()));
    }

    fn about_button<'ui>(&self, ui: &'ui imgui::Ui) -> bool {
        let mut ret = false;
        let sx = 120.0;
        let sy = 50.0;
        ui.window("_about_button_window")
            .position([self.windowsize.0 - sx * self.ui_scale - 10.0, 10.0], Condition::Always)
            .size([sx * self.ui_scale, sy * self.ui_scale], Condition::Always)
            .title_bar(false)
            .movable(false)
            .resizable(false)
            .collapsible(false)
            .build(|| {
                ui.set_window_font_scale(1.5 * self.ui_scale);
                ui.set_cursor_pos([10.0 * self.ui_scale, 10.0 * self.ui_scale]);
                if ui.button_with_size("About",[(sx - 20.0) * self.ui_scale, (sy - 20.0) * self.ui_scale]) {
                    ret = true;
                }
            });
            ;
        ret
    }

    fn window<'ui,'p>(&self, ui: &'ui imgui::Ui, name: &'static str, pos: (f32, f32), size: (f32, f32)) -> Window<'ui, 'p, &'static str> 
    where 'ui: 'p {
        ui.window(name)
            .position([self.ui_center.0 + pos.0 * self.ui_scale, self.ui_center.1 + pos.1 * self.ui_scale], Condition::Always)
            .size([size.0 * self.ui_scale, size.1 * self.ui_scale], Condition::Always)
            .title_bar(false)
            .movable(false)
            .resizable(false)
            .collapsible(false)
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
                    let dst = if idtagged {
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
        // TODO non-emscripten: load from local settings file?
        #[cfg(target_os = "emscripten")] {
            if let Some(new_idtag) = self.load_idtag.consume(|data| {
                let mut idtag = String::from_utf8(data).unwrap_or(client::gen_idtag());
                if idtag.len() != 32 {
                    idtag = client::gen_idtag();
                }
                idtag
            }, || {
                client::gen_idtag()
            }) {
                emscripten_util::localstorage::store("TETRIS", "idtag", &new_idtag.clone().into_bytes());
                self.server.set_idtag(&new_idtag);
                self.request_highscores();
            }
        }
    }

    fn check_player_data(&mut self) {
        // TODO non-emscripten: load from local settings file?
        #[cfg(target_os = "emscripten")] {
            if let Some(data) = self.load_player.consume(|data| {
                String::from_utf8(data).ok().and_then(|data| tetris::networking::decode(&data))
            }, || { None }) {
                if let Some(data) = data {
                    self.player = data;
                    self.config.level = self.player.level;
                    self.renderer.ghost_piece = self.player.ghost;
                    self.renderer.threed = self.player.render3d;
                }
            }
        }
    }

    fn save_player_data(&mut self) {
        #[cfg(target_os = "emscripten")]
            emscripten_util::localstorage::store("TETRIS", "player", tetris::networking::encode(&self.player).as_bytes());
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
            player: PlayerOptions {
                left: Keycode::Left as i32,
                right: Keycode::Right as i32,
                drop: Keycode::Down as i32,
                rotl: Keycode::Y as i32,
                rotr: Keycode::X as i32,
                pause: Keycode::Return as i32,
                level: 0,
                name: String::from("Your name please?"),
                sound: true,
                ghost: true,
                render3d: true,
            },
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
            #[cfg(target_os = "emscripten")] load_idtag: emscripten_util::localstorage::load("TETRIS", "idtag"),
            #[cfg(target_os = "emscripten")] load_player: emscripten_util::localstorage::load("TETRIS", "player"),
            scores_global: Vec::new(),
            scores_local: Vec::new(),
            last_global: Vec::new(),
            last_local: Vec::new(),
            replays: HashMap::new(),
            fpswidget: appbase::fpswidget::FpsWidget::new(180),
        };

        #[cfg(not(target_os = "emscripten"))] ret.server.set_idtag(&client::gen_idtag());
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
        self.fpswidget.push(dt);

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
            State::Game{mut game, paused, mut finished, mut dtime} => {
                if !finished && !paused {
                    // advance timer
                    dtime -= dt.min(0.1) - FRAME;

                    let mut frames = 1;
                    while dtime < -FRAME {
                        frames += 1;
                        dtime += FRAME;
                    }
                    while dtime > FRAME {
                        frames -= 1;
                        dtime -= FRAME;
                    }

                    for i in 0..frames {
                        if let Some(ret) = game.frame() {
                            match ret {
                                tetris::game::Outcome::Death => {
                                    self.save(&game);
                                    finished = true;
                                }
                                tetris::game::Outcome::HorizonalMove => {
                                    if self.player.sound { play_sound("blip"); }
                                }
                                tetris::game::Outcome::Merge => {
                                    if self.player.sound { play_sound("deww"); }
                                }
                                tetris::game::Outcome::Clear(..) => {
                                    if self.player.sound { play_sound("dabbedi"); }
                                }
                            }
                        }
                    }
                }

                self.renderer.set_state(game.timestamp(), game.snapshot());
                State::Game{game, paused, finished, dtime}
            },
            State::Replay{mut replayer} => {
                let adv = dt * replayer.speed;
                replayer.advance(adv);
                self.renderer.set_state(replayer.timestamp(), replayer.snapshot());
                State::Replay{replayer}
            },
            State::Highscores{selected, sort_by_score, global} => { bg = true; State::Highscores{selected, sort_by_score, global} },
            State::MainMenu => { bg = true; State::MainMenu },
            State::PreGame{keyconfig} => { bg = true; State::PreGame{keyconfig} },
            State::About => { bg = true; State::About },
        });

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

        // self.fpswidget.render(ui, (0.0, 0.0), (240.0, 80.0));

        self.ui = Some(match self.ui.take().unwrap() {
            State::MainMenu => {
                let mut ret = State::MainMenu;
                self.window(ui, "mainmenu_start", (mb1x, mby), (mbw, mbh)).build(|| {
                    ui.set_window_font_scale(2.0 * self.ui_scale);
                    ui.set_cursor_pos([20.0 * self.ui_scale, 20.0 * self.ui_scale]);
                    if ui.button_with_size("Start Game", [(mbw - 40.0)* self.ui_scale, (mbh - 40.0) * self.ui_scale]) {
                        self.check_player_data();
                        ret = State::PreGame{keyconfig: None};
                    }
                });
                self.window(ui, "mainmenu_highscores", (mb2x, mby), (mbw, mbh)).build(|| {
                    ui.set_window_font_scale(2.0 * self.ui_scale);
                    ui.set_cursor_pos([20.0 * self.ui_scale, 20.0 * self.ui_scale]);
                    if ui.button_with_size("Highscores", [(mbw - 40.0) * self.ui_scale, (mbh - 40.0) * self.ui_scale]) {
                        ret = State::Highscores{selected: None, sort_by_score: true, global: true};
                    }
                });
                if self.about_button(ui) {
                    ret = State::About;
                }
                ret
            },

            State::Highscores{mut selected, mut sort_by_score, mut global} => {
                let mut ret = None;

                self.window(ui, "highscores_back", (mb2x, mby), (mbw, mbh)).build(|| {
                    ui.set_window_font_scale(2.0 * self.ui_scale);
                    ui.set_cursor_pos([20.0 * self.ui_scale, 20.0 * self.ui_scale]);
                    if ui.button_with_size("Back", [(mbw - 40.0)* self.ui_scale, (mbh - 40.0) * self.ui_scale]) {
                        ret = Some(State::MainMenu);
                    }
                });

                self.window(ui, "highscores_list", (mb1x, mby - 150.0), (mb2x - mb1x, mbh + 300.0)).build(|| {
                    // get high-score list
                    let scores = if global {
                        if sort_by_score { &self.scores_global } else { &self.last_global }
                    } else {
                        if sort_by_score { &self.scores_local } else { &self.last_local }
                    };

                    ui.set_window_font_scale(1.2 * self.ui_scale);
                    ui.set_cursor_pos([20.0 * self.ui_scale, 20.0 * self.ui_scale]);
                    let btn = if sort_by_score { "By Score##highscores" } else { "By Date##highscores" };
                    if ui.button_with_size(btn, [100.0 * self.ui_scale, 30.0 * self.ui_scale]) {
                        sort_by_score = !sort_by_score;
                    }

                    if let Some(idx) = selected {
                        if idx < scores.len() {
                            ui.same_line_with_pos(0.5 * (mb2x - mb1x - 100.0) * self.ui_scale);
                            let replay_id = scores[idx].replay();
                            let replay = self.replays.get(&replay_id);
                            let btn = if replay.is_some() { "Replay##highscores" } else { "Downloading...##nighscores" };
                            if ui.button_with_size(btn, [100.0 * self.ui_scale, 30.0 * self.ui_scale]) {
                                if let Some(replay) = replay {
                                    ret = Some(State::Replay {
                                        replayer: tetris::replay::Replayer::new(replay)
                                    });
                                }
                            }
                        }
                    }

                    ui.same_line_with_pos((mb2x - mb1x - 120.0) * self.ui_scale);
                    let btn = if global { "Global##highscores" } else { "Local##highscores" };
                    if ui.button_with_size(btn, [100.0 * self.ui_scale, 30.0 * self.ui_scale]) {
                        global = !global;
                    }

                    ui.columns(5, "High-Scores List", true);
                    ui.separator();

                    ui.text("Score"); ui.next_column();
                    ui.text("Name"); ui.next_column();
                    ui.text("Lv Start"); ui.next_column();
                    ui.text("Lv End"); ui.next_column();
                    ui.text("Date"); ui.next_column();
                    ui.separator();

                    for score in scores.iter().enumerate() {
                        let label = format!("{}##entry{}", score.1.score().to_string(), score.0);
                        let was_selected = selected.map_or(false, |s| s == score.0);
                        let is_selected = ui.selectable_config(label)
                            .selected(was_selected)
                            .span_all_columns(true)
                            .size([0.0, 0.0])
                            .build();
                        if is_selected {
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

                if self.about_button(ui) {
                    ret = Some(State::About);
                }

                ret.unwrap_or(State::Highscores{selected, sort_by_score, global})
            }
            State::PreGame{mut keyconfig} => {
                let mut ret = None;

                self.window(ui, "pregame_start", (mb1x, mby), (mbw, mbh)).build(|| {
                    ui.set_window_font_scale(2.0 * self.ui_scale);
                    ui.set_cursor_pos([20.0 * self.ui_scale, 20.0 * self.ui_scale]);
                    if ui.button_with_size("Start", [(mbw - 40.0)* self.ui_scale, (mbh - 40.0) * self.ui_scale]) {
                        self.renderer.gen_new_colors();
                        self.save_player_data();
                        ret = Some(State::Game {
                            game: tetris::game::Game::new(&self.config),
                            paused: false,
                            finished: false,
                            dtime: 0.0,
                        });
                    }
                });

                self.window(ui, "pregame_back", (mb2x, mby), (mbw, mbh)).build(|| {
                    ui.set_window_font_scale(2.0 * self.ui_scale);
                    ui.set_cursor_pos([20.0 * self.ui_scale, 20.0 * self.ui_scale]);
                    if ui.button_with_size("Back", [(mbw - 40.0)* self.ui_scale, (mbh - 40.0) * self.ui_scale]) {
                        self.save_player_data();
                        ret = Some(State::MainMenu);
                    }
                });

                let optionswin = ((mb1x + mbw, mby - 120.0), (mb2x - mb1x - mbw, mbh + 240.0));
                self.window(ui, "pregame_options", optionswin.0, optionswin.1).build(|| {
                    ui.set_window_font_scale(1.5 * self.ui_scale);
                    if keyconfig.is_none() {
                        ui.set_cursor_pos([20.0 * self.ui_scale, 20.0 * self.ui_scale]);
                        ui.text("Starting Level");

                        ui.set_cursor_pos([20.0 * self.ui_scale, 45.0 * self.ui_scale]);
                        ui.push_item_width(mb2x - mb1x - mbw);
                        ui.slider("##pregamestartlevel", 0, 20, &mut self.config.level);
                        self.player.level = self.config.level;

                        ui.set_cursor_pos([20.0 * self.ui_scale, 85.0 * self.ui_scale]);
                        ui.text("Player Name");

                        ui.set_cursor_pos([20.0 * self.ui_scale, 110.0 * self.ui_scale]);
                        ui.push_item_width(mb2x - mb1x - mbw);
                        ui.input_text("##pregame_playername", &mut self.player.name).build();

                        ui.set_cursor_pos([20.0 * self.ui_scale, 165.0 * self.ui_scale]);
                        ui.checkbox("Ghost Piece", &mut self.renderer.ghost_piece);
                        self.player.ghost = self.renderer.ghost_piece;

                        ui.set_cursor_pos([20.0 * self.ui_scale, 210.0 * self.ui_scale]);
                        ui.checkbox("3D Pieces", &mut self.renderer.threed);
                        self.player.render3d = self.renderer.threed;

                        ui.set_cursor_pos([20.0 * self.ui_scale, 255.0 * self.ui_scale]);
                        ui.checkbox("Play Sounds", &mut self.player.sound);
                    } else {
                        let mut keynum = *keyconfig.as_ref().unwrap();

                        {
                            let mut keychoice = |y, name, key: &mut i32, index, scale| {
                                ui.set_cursor_pos([30.0 * scale, y * scale]);
                                ui.text(name);
                                ui.set_cursor_pos([170.0 * scale, y * scale - 10.0]);
                                let buttonstr = if keynum == index { String::from("...") } else { format!("{:?}", Keycode::from_i32(*key).unwrap_or(Keycode::Escape)) };
                                if ui.button_with_size(format!("{}", buttonstr), [100.0 * scale, 30.0 * scale]) {
                                    keynum = if keynum == index { -1 } else { index };
                                }
                            };
                            keychoice(30.0,  "Left",         &mut self.player.left,  0, self.ui_scale);
                            keychoice(70.0,  "Right",        &mut self.player.right, 1, self.ui_scale);
                            keychoice(110.0, "Down",         &mut self.player.drop,  2, self.ui_scale);
                            keychoice(150.0, "Rotate Left",  &mut self.player.rotl,  3, self.ui_scale);
                            keychoice(190.0, "Rotate Right", &mut self.player.rotr,  4, self.ui_scale);
                            keychoice(230.0, "Pause",        &mut self.player.pause, 5, self.ui_scale);
                        }

                        keyconfig = Some(keynum);
                    }

                    let buttonstr = if keyconfig.is_some() { "Game Settings" } else { "Controls" };
                    let buttonw = 200.0;
                    ui.set_cursor_pos([0.5 * self.ui_scale * ((optionswin.1).0 - buttonw), 295.0 * self.ui_scale]);
                    if ui.button_with_size(format!("{}", buttonstr), [buttonw * self.ui_scale, 40.0 * self.ui_scale]) {
                        keyconfig = match keyconfig {
                            None => Some(-1),
                            Some(n) => None,
                        };
                    }
                });

                if self.about_button(ui) {
                    ret = Some(State::About);
                }

                ret.unwrap_or(State::PreGame{keyconfig})
            }
            State::Game{game, paused, mut finished, dtime} => {
                self.renderer.do_ui(ui, self.ui_center, self.ui_scale);
                let mut ret = None;

                let bg = ui.push_style_color(StyleColor::WindowBg, [0.0; 4]);
                let border = ui.push_style_color(StyleColor::Border, [0.0; 4]);
                self.window(ui, "Playing Game UI#window", (200.0, 150.0), (200.0, 200.0)).build(|| {
                    ui.set_window_font_scale(1.5 * self.ui_scale);

                    if !finished {
                        if ui.button_with_size("Give up!##playing", [140.0 * self.ui_scale, 40.0 * self.ui_scale]) && !paused {
                            self.save(&game);
                            finished = true;
                        }
                    } else {
                        if ui.button_with_size("Back##fromgametomain", [140.0 * self.ui_scale, 40.0 * self.ui_scale]) {
                            ret = Some(State::MainMenu)
                        }
                    }

                    ui.new_line();

                    if finished && ui.button_with_size("Watch Replay##aftergamefinished", [140.0 * self.ui_scale, 40.0 * self.ui_scale]) {
                        ret = Some(State::Replay {
                            replayer: tetris::replay::Replayer::new(game.replay())
                        });
                    }
                });
                border.pop();
                bg.pop();

                if paused {
                    self.window(ui, "pausedplayinggame", (-100.0, -50.0), (200.0, 100.0)).build(|| {
                        ui.set_window_font_scale(2.5 * self.ui_scale);
                        ui.new_line(); ui.text("  Paused")
                    });
                }

                ret.unwrap_or(State::Game{game, paused, finished, dtime})
            }
            State::Replay{mut replayer} => {
                self.renderer.do_ui(ui, self.ui_center, self.ui_scale);
                let mut ret = None;

                self.window(ui, "Replayer UI#window", (200.0, 100.0), (200.0, 200.0)).build(|| {
                    ui.set_window_font_scale(1.2 * self.ui_scale);
                    ui.text("Speed");
                    ui.slider("##replayspeedslider", -20.0, 20.0, &mut replayer.speed);
                    ui.new_line();

                    ui.text("Time");
                    let mut curr = replayer.frame();
                    ui.slider("##replaytimeslider", 0.0, replayer.length(), &mut curr);
                    replayer.jump(curr);

                    ui.new_line();

                    if ui.button_with_size("Back##tomainmenu", [140.0 * self.ui_scale, 40.0 * self.ui_scale]) {
                        ret = Some(State::MainMenu);
                    }
                });

                if replayer.paused {
                    self.window(ui, "pausedplayinggame", (-100.0, -50.0), (200.0, 100.0)).build(|| {
                        ui.set_window_font_scale(2.5 * self.ui_scale);
                        ui.new_line(); ui.text("  Paused")
                    });
                }

                ret.unwrap_or(State::Replay{replayer})
            },
            State::About => {
                let sz = (300.0, 320.0);
                let mut ret = State::About;
                self.window(ui, "about_dialog", (-0.5 * sz.0, -0.5 * sz.1), sz).build(|| {
                    ui.set_window_font_scale(1.25 * self.ui_scale);
                    let lines = vec!(
                        "Written by Wieland Hagen",
                        "",
                        "Built using",
                        " - Rust",
                        " - emscripten",
                        " - WebGL",
                        " - imgui",
                        "",
                        "Info: wielandhagen@web.de",
                        "",
                        "Thanks for playing :)",
                    );
                    let mut y = 20.0;
                    for l in &lines {
                        ui.set_cursor_pos([20.0 * self.ui_scale, y * self.ui_scale]);
                        ui.text(l);
                        y += 20.0;
                    }

                    let buttonsz = (150.0, 40.0);
                    ui.set_cursor_pos([0.5 * self.ui_scale * (sz.0 - buttonsz.0), (sz.1 - buttonsz.1 - 20.0) * self.ui_scale]);
                    if ui.button_with_size("Cool, dude!", [buttonsz.0 * self.ui_scale, buttonsz.1 * self.ui_scale]) {
                        ret = State::MainMenu;
                    }
                });
                ret
            }
        });
    }

    fn event(&mut self, event: &Event) {
        match event {
            Event::KeyDown{keycode, keymod, .. } => {
                let ctrl = keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD);

                fn adjust_speed(value: &mut f32, delta: f32) {
                    let v = value.signum() * value.abs().sqrt() + delta;
                    *value = v.signum() * v.powi(2).min(20.0);
                }

                match &mut self.ui.as_mut().unwrap() {
                    State::Game{ref mut game, ref mut paused, ..} => {
                        let key =  keycode.unwrap() as i32;
                        if !*paused {
                            if key == self.player.left { game.left(true) }
                            if key == self.player.right { game.right(true) }
                            if key == self.player.drop { game.down(true) }
                            if key == self.player.rotl && !self.rotl {
                                self.rotl = true;
                                if self.player.sound { play_sound("rerr"); }
                                game.rotate(false);
                            }
                            if key == self.player.rotr && !self.rotr {
                                self.rotr = true;
                                if self.player.sound { play_sound("rerr"); }
                                game.rotate(true);
                            }
                        }
                        if key == self.player.pause { *paused = !*paused }
                    }
                    State::Replay { ref mut replayer } => match keycode.unwrap() {
                        Keycode::Left => replayer.advance(if ctrl { -10.0 } else { -1.0 }),
                        Keycode::Right => replayer.advance(if ctrl { 10.0 } else { 1.0 }),
                        Keycode::Up => adjust_speed(&mut replayer.speed, 0.1),
                        Keycode::Down => adjust_speed(&mut replayer.speed, -0.1),
                        Keycode::Return => replayer.paused = !replayer.paused,
                        _ => {}
                    }
                    State::PreGame { ref mut keyconfig } => {
                        if keyconfig.is_some() {
                            if let Some(num) = keyconfig.replace(-1) {
                                match num {
                                    0 => self.player.left = keycode.unwrap() as i32,
                                    1 => self.player.right = keycode.unwrap() as i32,
                                    2 => self.player.drop = keycode.unwrap() as i32,
                                    3 => self.player.rotl = keycode.unwrap() as i32,
                                    4 => self.player.rotr = keycode.unwrap() as i32,
                                    5 => self.player.pause = keycode.unwrap() as i32,
                                    _ => {}
                                }
                            }
                        }
;                    }
                    _ => {}
                }
            },
            Event::KeyUp{keycode, .. } => {
                match &mut self.ui.as_mut().unwrap() {
                    State::Game { ref mut game, .. } => {
                        let key =  keycode.unwrap() as i32;
                        if key == self.player.left { game.left(false) }
                        if key == self.player.right { game.right(false) }
                        if key == self.player.drop { game.down(false) }
                        if key == self.player.rotl { self.rotl = false; }
                        if key == self.player.rotr { self.rotr = false; }
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
