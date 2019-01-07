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

mod renderer;
mod util;
mod client;

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
    }
}

struct TetrisApp {
    windowsize: (f32, f32),
    fixedsize: (f32, f32),
    ui_center: (f32, f32),
    ui_scale: f32,

    ui: Option<State>,
    savegame: tetris::Savegame,
    config: tetris::Config,
    playername: String,

    renderer: renderer::Renderer,

    rotl: bool,
    rotr: bool,
}

impl TetrisApp {
    fn save(&mut self, game: &tetris::game::Game) {
        let last_breath = game.snapshot();
        self.savegame.add(game.replay().clone(), self.playername.clone(), last_breath.score(), last_breath.level());
        self.savegame.save("tetris.bin");
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
}

impl webrunner::WebApp for TetrisApp {
    fn new(windowsize: (u32, u32)) -> Self {
        TetrisApp {
            windowsize: (windowsize.0 as f32, windowsize.1 as f32),
            fixedsize: (710.0, 650.0),
            ui_scale: 1.0,
            ui_center: (0.5 * windowsize.0 as f32, 0.5 * windowsize.1 as f32),
            ui: Some(State::MainMenu),
            savegame: tetris::Savegame::load("tetris.bin"),
            config: tetris::Config::new(),
            playername: String::from("Wheelie :)"),
            renderer: renderer::Renderer::new(
                renderer::Rectangle::new(-150.0, -300.0, 300.0, 600.0),
                renderer::Rectangle::new(220.0, -300.0, 120.0, 120.0),
                renderer::Rectangle::new(220.0, -120.0, 180.0, 200.0),
                renderer::Rectangle::new(-400.0, -300.0, 300.0, 400.0),
            ),
            rotl: false,
            rotr: false,
        }
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
            State::Highscores{selected, sort_by_score} => { bg = true; State::Highscores{selected, sort_by_score} },
            State::MainMenu => { bg = true; State::MainMenu },
            State::PreGame => { bg = true; State::PreGame },
        });

        // Update Burn animation
        let nearfac = 0.1;
        let far = 10000.0;
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
                        ret = State::PreGame;
                    }
                });
                self.window(ui, im_str!("mainmenu_highscores"), (mb2x, mby), (mbw, mbh), 2.0).build(|| {
                    ui.set_cursor_pos((20.0 * self.ui_scale, 20.0 * self.ui_scale));
                    if ui.button(im_str!("Highscores"), ((mbw - 40.0) * self.ui_scale, (mbh - 40.0) * self.ui_scale)) {
                        ret = State::Highscores{selected: None, sort_by_score: true};
                    }
                });
                ret
            },

            State::Highscores{mut selected, mut sort_by_score} => {
                let mut ret = None;

                self.window(ui, im_str!("highscores_back"), (mb2x, mby), (mbw, mbh), 2.0).build(|| {
                    ui.set_cursor_pos((20.0 * self.ui_scale, 20.0 * self.ui_scale));
                    if ui.button(im_str!("Back"), ((mbw - 40.0)* self.ui_scale, (mbh - 40.0) * self.ui_scale)) {
                        ret = Some(State::MainMenu);
                    }
                });

                self.window(ui, im_str!("highscores_list"), (mb1x, mby - 150.0), (mb2x - mb1x, mbh + 300.0), 1.2).build(|| {
                    let scores = if sort_by_score { self.savegame.by_score() } else { self.savegame.by_date() };

                    ui.set_cursor_pos((20.0 * self.ui_scale, 20.0 * self.ui_scale));
                    if ui.button(im_str!("By Score##highscores"), (100.0 * self.ui_scale, 30.0 * self.ui_scale)) {
                        sort_by_score = true;
                    }

                    if let Some(idx) = selected {
                        ui.same_line(0.5 * (mb2x - mb1x - 100.0) * self.ui_scale);
                        if ui.button(im_str!("Replay##highscores"), (100.0 * self.ui_scale, 30.0 * self.ui_scale)) {
                            ret = Some(State::Replay {
                                replayer: tetris::replay::Replayer::new(scores[idx].replay())
                            });
                        }
                    }

                    ui.same_line((mb2x - mb1x - 120.0) * self.ui_scale);
                    if ui.button(im_str!("By Date##highscores"), (100.0 * self.ui_scale, 30.0 * self.ui_scale)) {
                        sort_by_score = false;
                    }

                    ui.columns(5, im_str!("High-Scores List"), true);
                    ui.separator();

                    ui.text("Score"); ui.next_column();
                    ui.text("Start Level"); ui.next_column();
                    ui.text("End Level"); ui.next_column();
                    ui.text("Name"); ui.next_column();
                    ui.text("Date"); ui.next_column();
                    ui.separator();

                    for score in scores.iter().enumerate() {
                        if ui.selectable(im_str!("{}##entry{}", score.1.score().to_string(), score.0),
                                         selected.map_or(false, |s| s == score.0),
                                         imgui::ImGuiSelectableFlags::SpanAllColumns,
                                         (0.0, 0.0)) {
                            selected = Some(score.0);
                        }
                        ui.next_column();
                        ui.text(score.1.replay().config().level.to_string()); ui.next_column();
                        ui.text(score.1.level().to_string()); ui.next_column();
                        ui.text(score.1.name().to_string()); ui.next_column();
                        ui.text(score.1.time_str()); ui.next_column();
                        ui.separator();
                    }
                });

                ret.unwrap_or(State::Highscores{selected, sort_by_score})
            }
            State::PreGame => {
                let mut ret = None;

                // TODO: enter name, check 'Wall Kick',

                self.window(ui, im_str!("pregame_start"), (mb1x, mby), (mbw, mbh), 2.0).build(|| {
                    ui.set_cursor_pos((20.0 * self.ui_scale, 20.0 * self.ui_scale));
                    if ui.button(im_str!("Start"), ((mbw - 40.0)* self.ui_scale, (mbh - 40.0) * self.ui_scale)) {
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
                });

                ret.unwrap_or(State::PreGame)
            }
            State::Game{game, paused, mut finished} => {
                self.renderer.do_ui(ui, self.ui_center, self.ui_scale);
                let mut ret = None;

                ui.with_color_var(imgui::ImGuiCol::WindowBg, (0.0, 0.0, 0.0, 0.0), || {
                    self.window(ui, im_str!("Playing Game UI#window"), (220.0, 200.0), (200.0, 200.0), 1.5).build(|| {
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
                    ui.slider_float(im_str!("##replayspeedslider"), &mut replayer.speed, -10.0, 10.0)
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
                    self.window(ui, im_str!("pausedplayinggame"), (280.0, 250.0), (180.0, 120.0), 2.5).build(|| {
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
                    let v = value.signum() * value.abs().sqrt() + delta;
                    *value = v.abs() * v;
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
    webrunner::AppRunner::<TetrisApp>::start("foo bar");
}
