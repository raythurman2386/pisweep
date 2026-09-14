//! GPUI Kit Minesweeper window. Rules and the hint AI live in the lib.

use std::borrow::Cow;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::label::Label;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{
    h_flex, v_flex, ActiveTheme, Icon, Root, Sizable as _, Theme, ThemeMode,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use pisweep::board::{format_time, pad3, Difficulty, Game, ThreadRng};
use pisweep::icons::SweepIcon;
use pisweep::solver::{self, Hint};
use pisweep::stats::{settings_paths, Settings, Store};
use pisweep::theme::{detect_system_dark, detect_text_scale, omarchy_watch_paths, OmarchyPalette};

actions!(
    pisweep_actions,
    [
        Reveal,
        Flag,
        Chord,
        NewGame,
        HintMove,
        Beginner,
        Intermediate,
        Expert,
        MoveLeft,
        MoveRight,
        MoveUp,
        MoveDown,
        ToggleHelp,
        ToggleFullscreen,
        Quit
    ]
);

pub fn init(cx: &mut App) {
    load_fonts(cx);
    cx.bind_keys([
        KeyBinding::new("space", Reveal, None),
        KeyBinding::new("enter", Reveal, None),
        KeyBinding::new("f", Flag, None),
        KeyBinding::new("x", Flag, None),
        KeyBinding::new("c", Chord, None),
        KeyBinding::new("n", NewGame, None),
        KeyBinding::new("a", HintMove, None),
        KeyBinding::new("1", Beginner, None),
        KeyBinding::new("2", Intermediate, None),
        KeyBinding::new("3", Expert, None),
        KeyBinding::new("left", MoveLeft, None),
        KeyBinding::new("h", MoveLeft, None),
        KeyBinding::new("right", MoveRight, None),
        KeyBinding::new("l", MoveRight, None),
        KeyBinding::new("up", MoveUp, None),
        KeyBinding::new("k", MoveUp, None),
        KeyBinding::new("down", MoveDown, None),
        KeyBinding::new("j", MoveDown, None),
        KeyBinding::new("shift-/", ToggleHelp, None),
        KeyBinding::new("f11", ToggleFullscreen, None),
        KeyBinding::new("super-f", ToggleFullscreen, None),
        KeyBinding::new("ctrl-q", Quit, None),
    ]);
}

fn load_fonts(cx: &mut App) {
    let fonts: [&'static [u8]; 4] = [
        include_bytes!("../fonts/iAWriterMonoS-Regular.ttf"),
        include_bytes!("../fonts/iAWriterMonoS-Italic.ttf"),
        include_bytes!("../fonts/iAWriterMonoS-Bold.ttf"),
        include_bytes!("../fonts/iAWriterMonoS-BoldItalic.ttf"),
    ];
    let blobs = fonts.into_iter().map(Cow::Borrowed).collect::<Vec<_>>();
    let _ = cx.text_system().add_fonts(blobs);
}

pub fn open_window(cx: &AsyncApp) -> anyhow::Result<WindowHandle<Root>> {
    cx.open_window(window_options(), move |window, cx| {
        let view: Entity<PisweepApp> = cx.new(|cx| PisweepApp::new(window, cx));
        cx.new(|cx| {
            let any_view: AnyView = view.into();
            Root::new(any_view, window, cx)
        })
    })
    .map_err(|e| anyhow::anyhow!("{e}"))
}

fn window_options() -> WindowOptions {
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds {
            origin: point(px(60.), px(40.)),
            size: size(px(920.), px(680.)),
        })),
        window_min_size: Some(size(px(480.), px(400.))),
        titlebar: Some(TitlebarOptions {
            title: Some("Pisweep".into()),
            appears_transparent: false,
            traffic_light_position: None,
        }),
        app_id: Some("pisweep".into()),
        ..Default::default()
    }
}

struct ThemeWatch {
    events: Arc<Mutex<Vec<std::path::PathBuf>>>,
    watcher: Option<RecommendedWatcher>,
    watched: Vec<std::path::PathBuf>,
}

impl ThemeWatch {
    fn new() -> Self {
        let events = Arc::new(Mutex::new(Vec::new()));
        let tx = events.clone();
        let watcher = RecommendedWatcher::new(
            move |res: notify::Result<notify::Event>| {
                if let Ok(event) = res {
                    if matches!(
                        event.kind,
                        EventKind::Modify(_) | EventKind::Remove(_) | EventKind::Create(_)
                    ) {
                        if let Ok(mut queue) = tx.lock() {
                            queue.extend(event.paths);
                        }
                    }
                }
            },
            notify::Config::default(),
        )
        .ok();
        let mut this = Self {
            events,
            watcher,
            watched: Vec::new(),
        };
        this.watch_all(omarchy_watch_paths());
        this
    }

    fn watch_all(&mut self, paths: impl IntoIterator<Item = std::path::PathBuf>) {
        self.unwatch_all();
        for path in paths {
            self.watch_path(&path);
        }
    }

    fn watch_path(&mut self, path: &std::path::Path) {
        if !path.exists() || self.watched.iter().any(|p| p == path) {
            return;
        }
        if let Some(watcher) = self.watcher.as_mut() {
            if watcher.watch(path, RecursiveMode::NonRecursive).is_ok() {
                self.watched.push(path.to_path_buf());
            }
        }
    }

    fn unwatch_all(&mut self) {
        if let Some(watcher) = self.watcher.as_mut() {
            for path in self.watched.drain(..) {
                let _ = watcher.unwatch(&path);
            }
        } else {
            self.watched.clear();
        }
    }

    fn drain(&self) -> Vec<std::path::PathBuf> {
        self.events
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    fn needs_rearm(&self) -> bool {
        self.watched.iter().any(|path| !path.exists())
    }
}

pub struct PisweepApp {
    palette: OmarchyPalette,
    text_scale: f32,
    theme_watch: ThemeWatch,
    focus_handle: FocusHandle,
    game: Game,
    rng: ThreadRng,
    started: Option<Instant>,
    frozen_elapsed: u32,
    hint_message: String,
    show_help: bool,
    settings: Settings,
    store: Store,
    _appearance_sub: Subscription,
    _poll_task: Task<()>,
}

impl Focusable for PisweepApp {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl PisweepApp {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let dark = detect_system_dark();
        let palette = OmarchyPalette::load(dark);
        let text_scale = detect_text_scale();
        let focus_handle = cx.focus_handle();
        let (_, path) = settings_paths();
        let store = Store::new(path);
        let settings = store.load();
        let difficulty = Difficulty::parse(&settings.difficulty);
        let game = Game::new(difficulty);

        apply_palette(&palette, Some(window), cx);
        window.set_window_title("Pisweep");

        let (appearance_sub, poll_task) = Self::start_theme_poll(window, cx);

        let this = Self {
            palette,
            text_scale,
            theme_watch: ThemeWatch::new(),
            focus_handle,
            game,
            rng: ThreadRng::default(),
            started: None,
            frozen_elapsed: 0,
            hint_message: String::new(),
            show_help: false,
            settings,
            store,
            _appearance_sub: appearance_sub,
            _poll_task: poll_task,
        };
        let handle = this.focus_handle.clone();
        window.focus(&handle, cx);
        this
    }

    fn start_theme_poll(window: &mut Window, cx: &mut Context<Self>) -> (Subscription, Task<()>) {
        let appearance = cx.observe_window_appearance(window, |this, window, cx| {
            this.poll_theme(window, cx);
        });
        let poll = cx.spawn_in(window, async move |this, cx| loop {
            cx.background_executor()
                .timer(Duration::from_millis(200))
                .await;
            if this
                .update_in(cx, |this, window, cx| {
                    this.poll_theme(window, cx);
                    if this.started.is_some() && !this.game.over {
                        cx.notify();
                    }
                })
                .is_err()
            {
                break;
            }
        });
        (appearance, poll)
    }

    fn poll_theme(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let events = self.theme_watch.drain();
        if !events.is_empty() || self.theme_watch.needs_rearm() {
            self.theme_watch.watch_all(omarchy_watch_paths());
        }
        let palette = OmarchyPalette::load(detect_system_dark());
        if palette != self.palette {
            self.palette = palette;
            apply_palette(&self.palette, Some(window), cx);
            cx.notify();
        }
        let scale = detect_text_scale();
        if (scale - self.text_scale).abs() > f32::EPSILON {
            self.text_scale = scale;
            apply_palette(&self.palette, Some(window), cx);
            cx.notify();
        }
    }

    fn elapsed(&self) -> u32 {
        if self.game.over {
            self.frozen_elapsed
        } else if let Some(start) = self.started {
            start.elapsed().as_secs() as u32
        } else {
            0
        }
    }

    fn note_start(&mut self) {
        if self.started.is_none() && self.game.placed && !self.game.over {
            self.started = Some(Instant::now());
        }
        if self.game.over {
            if self.frozen_elapsed == 0 {
                self.frozen_elapsed = self
                    .started
                    .map(|t| t.elapsed().as_secs() as u32)
                    .unwrap_or(0);
            }
            if self.game.won
                && self
                    .settings
                    .bests
                    .record(self.game.difficulty, self.frozen_elapsed)
            {
                self.settings.difficulty = self.game.difficulty.id().into();
                self.store.save(&self.settings);
            }
        }
    }

    fn new_game(&mut self, difficulty: Difficulty, cx: &mut Context<Self>) {
        self.game = Game::new(difficulty);
        self.started = None;
        self.frozen_elapsed = 0;
        self.hint_message.clear();
        self.settings.difficulty = difficulty.id().into();
        self.store.save(&self.settings);
        cx.notify();
    }

    fn reveal_at(&mut self, x: u8, y: u8, cx: &mut Context<Self>) {
        self.game.set_cursor(x, y);
        self.game.reveal(x, y, &mut self.rng);
        self.hint_message.clear();
        self.note_start();
        cx.notify();
    }

    fn flag_at(&mut self, x: u8, y: u8, cx: &mut Context<Self>) {
        self.game.set_cursor(x, y);
        self.game.flag(x, y);
        self.hint_message.clear();
        cx.notify();
    }

    fn chord_at(&mut self, x: u8, y: u8, cx: &mut Context<Self>) {
        self.game.set_cursor(x, y);
        self.game.chord(x, y, &mut self.rng);
        self.hint_message.clear();
        self.note_start();
        cx.notify();
    }

    fn play_hint(&mut self, cx: &mut Context<Self>) {
        if self.game.over {
            return;
        }
        match solver::hint(&self.game) {
            Hint::None => {
                self.hint_message = "No certain move".into();
                cx.notify();
            }
            Hint::Reveal { x, y } => self.reveal_at(x, y, cx),
            Hint::Flag { x, y } => self.flag_at(x, y, cx),
        }
    }
}

impl Render for PisweepApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let page = hex_to_hsla(&self.palette.background).unwrap_or_else(|| cx.theme().background);
        let ink = hex_to_hsla(&self.palette.foreground).unwrap_or_else(|| cx.theme().foreground);
        let muted = hex_to_hsla(&self.palette.muted).unwrap_or_else(|| cx.theme().muted_foreground);
        let surface = hex_to_hsla(&self.palette.surface).unwrap_or_else(|| cx.theme().secondary);
        let accent = hex_to_hsla(&self.palette.accent).unwrap_or_else(|| cx.theme().accent);
        let scale = self.text_scale;
        let cols = self.game.width as usize;
        let rows = self.game.height as usize;
        let cell = cell_px(cols, scale);
        let gap = if cols >= 24 { 1.0 } else { 2.0 } * scale;
        let cursor = (self.game.cursor_x, self.game.cursor_y);
        let over = self.game.over;
        let won = self.game.won;
        let remaining = pad3(self.game.remaining());
        let elapsed = format_time(self.elapsed());
        let best = self.settings.bests.get(self.game.difficulty);
        let best_label = if best > 0 {
            format_time(best)
        } else {
            "—".into()
        };
        let face = if won {
            SweepIcon::Grin
        } else if over {
            SweepIcon::Frown
        } else {
            SweepIcon::Smile
        };
        let difficulty = self.game.difficulty;
        let help = self.show_help;
        let icon_px = (cell * 0.52).max(11.0);
        let mines_tip = format!(
            "{} mines remaining of {}",
            self.game.remaining(),
            self.game.mines
        );
        let time_tip = format!("Elapsed {elapsed}");
        let best_tip = if best > 0 {
            format!("Best {} time {}", difficulty.label(), best_label)
        } else {
            format!("No best time for {}", difficulty.label())
        };
        let face_tip = if won {
            "You won — new game"
        } else if over {
            "You lost — new game"
        } else {
            "New game"
        };
        let ui_tone = (page, ink, scale);
        window.set_window_title(&format!(
            "Pisweep — {}{}",
            difficulty.label(),
            if won {
                " (won)"
            } else if over {
                " (lost)"
            } else {
                ""
            }
        ));

        let header = h_flex()
            .id("hud")
            .w_full()
            .px_4()
            .py_2()
            .gap_3()
            .items_center()
            .flex_wrap()
            .bg(surface)
            .border_b_1()
            .border_color(page.blend(ink.opacity(0.08)))
            .child(hud_chip(
                "mines",
                SweepIcon::Flag,
                accent,
                remaining,
                mines_tip,
                ui_tone,
            ))
            .child(
                Button::new("face")
                    .ghost()
                    .icon(face)
                    .tooltip_with_action(face_tip, &NewGame, Some("pisweep"))
                    .accessibility_label(face_tip)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.new_game(this.game.difficulty, cx);
                    })),
            )
            .child(hud_chip(
                "timer",
                SweepIcon::Timer,
                ink,
                elapsed,
                time_tip,
                ui_tone,
            ))
            .child(hud_chip(
                "best",
                SweepIcon::Trophy,
                muted,
                best_label,
                best_tip,
                ui_tone,
            ))
            .child(
                h_flex()
                    .gap_1()
                    .children(Difficulty::all().into_iter().map(|d| {
                        let (w, h, mines) = d.spec();
                        let tip = format!("{} — {w}×{h}, {mines} mines", d.label());
                        let mut btn = Button::new(SharedString::from(d.id()))
                            .label(d.label())
                            .accessibility_label(tip.clone())
                            .when(d == difficulty, |b| b.primary())
                            .when(d != difficulty, |b| b.ghost())
                            .on_click(cx.listener(move |this, _, _, cx| this.new_game(d, cx)));
                        btn = match d {
                            Difficulty::Beginner => {
                                btn.tooltip_with_action(tip, &Beginner, Some("pisweep"))
                            }
                            Difficulty::Intermediate => {
                                btn.tooltip_with_action(tip, &Intermediate, Some("pisweep"))
                            }
                            Difficulty::Expert => {
                                btn.tooltip_with_action(tip, &Expert, Some("pisweep"))
                            }
                        };
                        btn
                    })),
            )
            .child(
                Button::new("hint")
                    .ghost()
                    .icon(SweepIcon::Lightbulb)
                    .label("Hint")
                    .tooltip_with_action("Play one certain solver move", &HintMove, Some("pisweep"))
                    .accessibility_label("Hint, play one certain move")
                    .on_click(cx.listener(|this, _, _, cx| this.play_hint(cx))),
            );

        let board = v_flex()
            .id("board")
            .gap(px(gap))
            .p_4()
            .rounded_lg()
            .bg(surface)
            .border_1()
            .border_color(page.blend(ink.opacity(0.08)))
            .aria_label(format!(
                "Minesweeper board, {} by {}, {} mines",
                cols, rows, self.game.mines
            ))
            .children((0..rows).map(|y| {
                h_flex()
                    .id(SharedString::from(format!("row-{y}")))
                    .gap(px(gap))
                    .children((0..cols).map(|x| {
                        let cell_data = self.game.cells[y * cols + x];
                        let selected = cursor == (x as u8, y as u8);
                        let look = cell_look(&cell_data, over, won, ink, muted, accent, page);
                        let xx = x as u8;
                        let yy = y as u8;
                        let mark = look.mark;
                        let announce = cell_announcement(&cell_data, over, won, selected);
                        div()
                            .id(SharedString::from(format!("c-{x}-{y}")))
                            .w(px(cell))
                            .h(px(cell))
                            .rounded_md()
                            .flex()
                            .items_center()
                            .justify_center()
                            .aria_label(announce.clone())
                            .tooltip({
                                let tip = announce.clone();
                                move |window, cx| Tooltip::new(tip.clone()).build(window, cx)
                            })
                            .bg(look.fill)
                            .border_1()
                            .border_color(if selected {
                                accent
                            } else if matches!(mark, CellMark::Blank) && !cell_data.revealed {
                                page.blend(ink.opacity(0.16))
                            } else {
                                page.blend(ink.opacity(0.04))
                            })
                            .cursor_pointer()
                            .hover(|el| {
                                if !cell_data.revealed {
                                    el.bg(page.blend(accent.opacity(0.10)))
                                } else {
                                    el
                                }
                            })
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _, _, cx| this.reveal_at(xx, yy, cx)),
                            )
                            .on_mouse_down(
                                MouseButton::Right,
                                cx.listener(move |this, _, _, cx| this.flag_at(xx, yy, cx)),
                            )
                            .on_mouse_down(
                                MouseButton::Middle,
                                cx.listener(move |this, _, _, cx| this.chord_at(xx, yy, cx)),
                            )
                            .child(match mark {
                                CellMark::Blank => div().into_any_element(),
                                CellMark::Digit(n) => Label::new(n.to_string())
                                    .text_size(px((cell * 0.52).max(11.)))
                                    .text_color(look.color)
                                    .into_any_element(),
                                CellMark::Icon(icon) => Icon::new(icon)
                                    .with_size(px(icon_px))
                                    .text_color(look.color)
                                    .into_any_element(),
                            })
                    }))
            }));

        let status_text = if self.hint_message.is_empty() {
            "space open · f flag · c chord · a hint · n new · 1/2/3 difficulty · ? help".into()
        } else {
            self.hint_message.clone()
        };
        let footer = h_flex()
            .id("status")
            .w_full()
            .px_3()
            .py_2()
            .bg(surface)
            .border_t_1()
            .border_color(page.blend(ink.opacity(0.08)))
            .aria_label(status_text.clone())
            .child(Label::new(status_text).text_sm().text_color(muted));

        let overlay = help.then(|| {
            div()
                .id("help")
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .bg(page.opacity(0.72))
                .aria_label("Keyboard shortcuts")
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| {
                        this.show_help = false;
                        cx.notify();
                    }),
                )
                .child(
                    v_flex()
                        .id("help-card")
                        .w(px(420. * scale))
                        .p_5()
                        .gap_2()
                        .rounded_lg()
                        .bg(surface)
                        .border_1()
                        .border_color(accent.opacity(0.4))
                        .child(
                            h_flex()
                                .gap_2()
                                .items_center()
                                .child(Icon::new(SweepIcon::Info).small().text_color(accent))
                                .child(
                                    Label::new("Keys")
                                        .text_size(px(20. * scale))
                                        .text_color(ink),
                                ),
                        )
                        .child(
                            Label::new("space / enter  open")
                                .text_sm()
                                .text_color(muted),
                        )
                        .child(
                            Label::new("f / x          flag")
                                .text_sm()
                                .text_color(muted),
                        )
                        .child(
                            Label::new("c              chord")
                                .text_sm()
                                .text_color(muted),
                        )
                        .child(
                            Label::new("arrows / hjkl  cursor")
                                .text_sm()
                                .text_color(muted),
                        )
                        .child(
                            Label::new("a              certain hint")
                                .text_sm()
                                .text_color(muted),
                        )
                        .child(
                            Label::new("n              new game")
                                .text_sm()
                                .text_color(muted),
                        )
                        .child(
                            Label::new("1 2 3          beginner / inter / expert")
                                .text_sm()
                                .text_color(muted),
                        )
                        .child(
                            Label::new("Ctrl+Q         quit")
                                .text_sm()
                                .text_color(muted),
                        ),
                )
        });

        v_flex()
            .id("pisweep")
            .size_full()
            .relative()
            .bg(page)
            .font_family("iA Writer Mono S")
            .text_size(px(16. * scale))
            .key_context("pisweep")
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(|this, _: &Reveal, _, cx| {
                this.reveal_at(this.game.cursor_x, this.game.cursor_y, cx);
            }))
            .on_action(cx.listener(|this, _: &Flag, _, cx| {
                this.flag_at(this.game.cursor_x, this.game.cursor_y, cx);
            }))
            .on_action(cx.listener(|this, _: &Chord, _, cx| {
                this.chord_at(this.game.cursor_x, this.game.cursor_y, cx);
            }))
            .on_action(cx.listener(|this, _: &NewGame, _, cx| {
                this.new_game(this.game.difficulty, cx);
            }))
            .on_action(cx.listener(|this, _: &HintMove, _, cx| this.play_hint(cx)))
            .on_action(cx.listener(|this, _: &Beginner, _, cx| {
                this.new_game(Difficulty::Beginner, cx);
            }))
            .on_action(cx.listener(|this, _: &Intermediate, _, cx| {
                this.new_game(Difficulty::Intermediate, cx);
            }))
            .on_action(cx.listener(|this, _: &Expert, _, cx| {
                this.new_game(Difficulty::Expert, cx);
            }))
            .on_action(cx.listener(|this, _: &MoveLeft, _, cx| {
                this.game.move_cursor(-1, 0);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &MoveRight, _, cx| {
                this.game.move_cursor(1, 0);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &MoveUp, _, cx| {
                this.game.move_cursor(0, -1);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &MoveDown, _, cx| {
                this.game.move_cursor(0, 1);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &ToggleHelp, _, cx| {
                this.show_help = !this.show_help;
                cx.notify();
            }))
            .on_action(cx.listener(|_, _: &ToggleFullscreen, window, _| {
                window.toggle_fullscreen();
            }))
            .on_action(cx.listener(|_, _: &Quit, _, cx| cx.quit()))
            .child(header)
            .child(
                v_flex()
                    .id("board-host")
                    .flex_1()
                    .items_center()
                    .justify_center()
                    .overflow_scroll()
                    .child(board),
            )
            .child(footer)
            .children(overlay)
    }
}

fn hud_chip(
    id: &'static str,
    icon: SweepIcon,
    color: Hsla,
    label: String,
    tip: String,
    tone: (Hsla, Hsla, f32),
) -> impl IntoElement {
    let (page, ink, scale) = tone;
    h_flex()
        .id(id)
        .gap_1p5()
        .items_center()
        .px_2()
        .py_1()
        .rounded_md()
        .bg(page.blend(ink.opacity(0.06)))
        .aria_label(tip.clone())
        .tooltip({
            let tip = tip.clone();
            move |window, cx| Tooltip::new(tip.clone()).build(window, cx)
        })
        .child(Icon::new(icon).small().text_color(color))
        .child(
            Label::new(label)
                .text_size(px(18. * scale))
                .text_color(color),
        )
}

fn cell_announcement(cell: &pisweep::board::Cell, over: bool, won: bool, selected: bool) -> String {
    let pos = format!("row {}, column {}", cell.y + 1, cell.x + 1);
    let state = if over && !won && cell.flagged && !cell.mine {
        "wrong flag"
    } else if cell.flagged && !cell.revealed {
        "flagged"
    } else if !cell.revealed {
        "covered"
    } else if cell.mine && cell.exploded {
        "exploded mine"
    } else if cell.mine {
        "mine"
    } else if cell.adj == 0 {
        "empty"
    } else if cell.adj == 1 {
        "1 adjacent mine"
    } else {
        return format!(
            "{} adjacent mines, {pos}{}",
            cell.adj,
            if selected { ", selected" } else { "" }
        );
    };
    format!("{state}, {pos}{}", if selected { ", selected" } else { "" })
}

fn cell_px(cols: usize, scale: f32) -> f32 {
    let base = if cols >= 24 {
        16.0
    } else if cols >= 16 {
        22.0
    } else {
        28.0
    };
    (base * scale).max(12.0)
}

#[derive(Clone, Copy)]
enum CellMark {
    Blank,
    Digit(u8),
    Icon(SweepIcon),
}

struct CellLook {
    mark: CellMark,
    fill: Hsla,
    color: Hsla,
}

fn cell_look(
    cell: &pisweep::board::Cell,
    over: bool,
    won: bool,
    ink: Hsla,
    muted: Hsla,
    accent: Hsla,
    page: Hsla,
) -> CellLook {
    let covered = page.blend(ink.opacity(0.12));
    let open = page.blend(ink.opacity(0.04));
    if over && !won && cell.flagged && !cell.mine {
        return CellLook {
            mark: CellMark::Icon(SweepIcon::X),
            fill: page.blend(rgb(0xef4444).into()).opacity(0.18),
            color: rgb(0xef4444).into(),
        };
    }
    if cell.flagged && !cell.revealed {
        return CellLook {
            mark: CellMark::Icon(SweepIcon::Flag),
            fill: covered,
            color: accent,
        };
    }
    if !cell.revealed {
        return CellLook {
            mark: CellMark::Blank,
            fill: covered,
            color: ink,
        };
    }
    if cell.mine {
        return CellLook {
            mark: CellMark::Icon(SweepIcon::Bomb),
            fill: if cell.exploded {
                rgb(0xb91c1c).into()
            } else {
                open
            },
            color: if cell.exploded {
                rgb(0xfff7ed).into()
            } else {
                ink
            },
        };
    }
    let color = match cell.adj {
        1 => accent,
        2 => rgb(0x22c55e).into(),
        3 => rgb(0xef4444).into(),
        4 => rgb(0x3b82f6).into(),
        5 => rgb(0xb45309).into(),
        6 => rgb(0x0d9488).into(),
        7 => ink,
        _ => muted,
    };
    CellLook {
        mark: if cell.adj == 0 {
            CellMark::Blank
        } else {
            CellMark::Digit(cell.adj)
        },
        fill: open,
        color,
    }
}

fn apply_palette(palette: &OmarchyPalette, window: Option<&mut Window>, cx: &mut App) {
    Theme::change(
        if palette.dark {
            ThemeMode::Dark
        } else {
            ThemeMode::Light
        },
        window,
        cx,
    );
    let theme = Theme::global_mut(cx);
    if let Some(bg) = hex_to_hsla(&palette.background) {
        theme.colors.background = bg;
    }
    if let Some(fg) = hex_to_hsla(&palette.foreground) {
        theme.colors.foreground = fg;
    }
    if let Some(accent) = hex_to_hsla(&palette.accent) {
        theme.colors.primary = accent;
        theme.colors.accent = accent;
    }
    if let Some(surface) = hex_to_hsla(&palette.surface) {
        theme.colors.secondary = surface;
    }
    if let Some(muted) = hex_to_hsla(&palette.muted) {
        theme.colors.muted_foreground = muted;
    }
    theme.mono_font_family = "iA Writer Mono S".into();
    theme.mono_font_size = px(16.);
    Theme::sync_base(cx);
}

fn hex_to_hsla(value: &str) -> Option<Hsla> {
    let hex = value.trim().trim_start_matches('#');
    let expanded = if hex.len() == 3 {
        hex.chars().flat_map(|c| [c, c]).collect::<String>()
    } else {
        hex.to_string()
    };
    let n = u32::from_str_radix(&expanded, 16).ok()?;
    Some(rgb(n).into())
}

#[cfg(test)]
mod tests {
    use super::{cell_announcement, hex_to_hsla};
    use pisweep::board::Cell;

    #[test]
    fn hex_to_hsla_parses_three_and_six_digit_colors() {
        assert!(hex_to_hsla("#101010").is_some());
        assert!(hex_to_hsla("#fff").is_some());
        assert!(hex_to_hsla("nope").is_none());
    }

    fn cell(x: u8, y: u8) -> Cell {
        Cell {
            x,
            y,
            mine: false,
            adj: 0,
            revealed: false,
            flagged: false,
            exploded: false,
        }
    }

    #[test]
    fn cell_announcement_covers_states() {
        let covered = cell(2, 4);
        assert_eq!(
            cell_announcement(&covered, false, false, true),
            "covered, row 5, column 3, selected"
        );
        let mut flagged = covered;
        flagged.flagged = true;
        assert!(cell_announcement(&flagged, false, false, false).starts_with("flagged"));
        let mut numbered = cell(0, 0);
        numbered.revealed = true;
        numbered.adj = 3;
        assert_eq!(
            cell_announcement(&numbered, false, false, false),
            "3 adjacent mines, row 1, column 1"
        );
    }
}
