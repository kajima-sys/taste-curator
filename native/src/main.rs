//! Taste Curator, natively: a shelf of references, a canvas you mark up, and an inspector that
//! turns each marking into structured taste. `--snap out.png` renders it headless.

// A desktop app, not a console program: on Windows this sets the GUI subsystem so no black
// terminal window opens beside the window. `--snap` / `--script` still work when launched from a
// shell (the console just gets no output).
#![cfg_attr(windows, windows_subsystem = "windows")]

mod app;
mod calibrate_view;
mod canvas;
mod files;
mod inspector;
mod shelf;
mod store;
mod theme;

use app::{App, Workspace};
use rune::tok::type_;
use rune::{col, el, row, text, AppSpec, Color, Cx, El, Key, KeyEvent, TextStyle, Tx};
use taste_curator::model::FlinchVerdict;
use taste_curator::tools::Tool;
use theme::Palette;

fn view(s: &App, cx: &mut Cx<App>) -> El {
    let p = theme::palette(s.dark);
    cx.foreground(p.ink_primary);

    let mode_switch = row()
        .key("mode-switch")
        .padding(3.0)
        .gap(2.0)
        .radius(7.0)
        .fill(p.bg_canvas)
        .border(1.0, p.border_hair)
        .items_center()
        .child(mode_tab(
            "mode-calibrate",
            "Calibrate",
            s.workspace == Workspace::Calibrate,
            p,
            Workspace::Calibrate,
        ))
        .child(mode_tab(
            "mode-markup",
            "Mark up",
            s.workspace == Workspace::MarkUp,
            p,
            Workspace::MarkUp,
        ));

    let bar = row()
        .key("topbar")
        .w_full()
        .h(46.0)
        .px(14.0)
        .gap(12.0)
        .items_center()
        .fill(p.bg_rail)
        .border_sides([0.0, 0.0, 1.0, 0.0], p.border_hair)
        .child(
            row()
                .key("brand")
                .gap(8.0)
                .items_center()
                .child(theme::dot(8.0, p.accent))
                .child(
                    text("Taste Curator")
                        .font(TextStyle::new(13.0, 600.0))
                        .color(p.ink_primary),
                )
                .child(
                    text("/")
                        .font(TextStyle::new(12.0, 400.0))
                        .color(p.border_strong),
                )
                .child(
                    text(&s.project.name)
                        .key("project-name")
                        .font(TextStyle::new(12.0, 400.0))
                        .color(p.ink_secondary)
                        .max_w(170.0)
                        .truncate(),
                ),
        )
        .child(mode_switch)
        .child(el().grow(1.0))
        .child(toolbar(s, p))
        .child(files::export_button(s, p));

    let center = match s.workspace {
        Workspace::Calibrate => calibrate_view::stage_view(s, cx),
        Workspace::MarkUp => canvas_pane(s, p),
    };
    let right = match s.workspace {
        Workspace::Calibrate => calibrate_view::envelope_inspector(s, cx),
        Workspace::MarkUp => inspector::view(s, cx),
    };

    col()
        .key("stage")
        .w_full()
        .h_full()
        .grow(1.0)
        .fill(p.bg_canvas)
        .child(bar)
        .child(
            row()
                .key("body")
                .w_full()
                .grow(1.0)
                .child(shelf::view(s, cx))
                .child(
                    col()
                        .key("main")
                        .grow(1.0)
                        .h_full()
                        .child(center)
                        .child(footer(s, p)),
                )
                .child(right),
        )
}

fn mode_tab(
    key: &'static str,
    label: &'static str,
    on: bool,
    p: Palette,
    target: Workspace,
) -> El {
    text(label)
        .key(key)
        .role("button")
        .focusable(true)
        .font(TextStyle::new(11.5, if on { 500.0 } else { 400.0 }))
        .color(if on { p.ink_primary } else { p.ink_secondary })
        .px(11.0)
        .py(4.0)
        .radius(5.0)
        .fill(if on { p.bg_elevated } else { Color::TRANSPARENT })
        .border(1.0, if on { p.border_strong } else { Color::TRANSPARENT })
        .hover(p.hover)
        .on_press(move |s: &mut App| {
            s.workspace = target;
            if target == Workspace::Calibrate {
                s.refresh_probe();
            }
        })
}

fn tool_key(tool: Tool) -> &'static str {
    match tool {
        Tool::Select => "V",
        Tool::Pan => "H",
        Tool::Rect => "R",
        Tool::Ellipse => "O",
        Tool::Freehand => "F",
    }
}

fn toolbar(s: &App, p: Palette) -> El {
    let mut out = row().key("tools").gap(6.0).items_center();
    if s.workspace == Workspace::MarkUp {
        let mut group = row()
            .key("tool-group")
            .padding(2.0)
            .gap(2.0)
            .radius(6.0)
            .fill(p.bg_canvas)
            .border(1.0, p.border_hair)
            .items_center();
        for tool in Tool::ALL {
            let on = s.tool == tool;
            group = group.child(
                row()
                    .key(format!("tool-{}", tool.label().to_lowercase()))
                    .role("button")
                    .label(format!("{} tool ({})", tool.label(), tool_key(tool)))
                    .focusable(true)
                    .gap(5.0)
                    .items_center()
                    .px(7.0)
                    .py(3.5)
                    .radius(4.5)
                    .fill(if on { p.bg_elevated } else { Color::TRANSPARENT })
                    .border(1.0, if on { p.border_strong } else { Color::TRANSPARENT })
                    .hover(p.hover)
                    .on_press(move |s: &mut App| {
                        s.tool = tool;
                        s.mode = app::Mode::Idle;
                        s.status = tool.hint().to_string();
                    })
                    .child(
                        text(tool.label())
                            .font(TextStyle::new(11.0, if on { 500.0 } else { 400.0 }))
                            .color(if on { p.ink_primary } else { p.ink_secondary }),
                    )
                    .child(theme::kbd(tool_key(tool), p)),
            );
        }
        out = out.child(group);
    }
    out = out.child(
        text(if s.dark { "Light" } else { "Dark" })
            .key("theme-toggle")
            .role("button")
            .label("Toggle dark or light theme")
            .focusable(true)
            .font(TextStyle::new(11.0, 500.0))
            .color(p.ink_secondary)
            .px(8.0)
            .py(5.0)
            .radius(6.0)
            .fill(p.bg_surface)
            .border(1.0, p.border_hair)
            .hover(p.hover)
            .on_press(|s: &mut App| s.dark = !s.dark),
    );
    out
}

fn canvas_pane(s: &App, p: Palette) -> El {
    let mut pane = col()
        .key("canvas-pane")
        .w_full()
        .grow(1.0)
        .padding(12.0)
        .fill(p.bg_canvas)
        .child(canvas::canvas_view(s));
    if s.artifact().is_none() {
        pane = pane.child(
            el()
                .key("empty")
                .absolute(0.0, 0.0)
                .w_full()
                .h_full()
                .center()
                .child(
                    text("Import a reference, then mark up what you like.")
                        .font(TextStyle::new(12.5, 400.0))
                        .color(p.ink_muted),
                ),
        );
    }
    pane
}

fn footer(s: &App, p: Palette) -> El {
    let mut out = row()
        .key("footer")
        .w_full()
        .h(32.0)
        .px(14.0)
        .gap(10.0)
        .items_center()
        .fill(p.bg_rail)
        .border_sides([1.0, 0.0, 0.0, 0.0], p.border_hair);

    if s.workspace == Workspace::Calibrate {
        out = out.child(
            row()
                .key("calibrate-hints")
                .gap(10.0)
                .items_center()
                .child(
                    row()
                        .gap(4.0)
                        .items_center()
                        .child(theme::kbd("1", p))
                        .child(
                            text("Still has it")
                                .font(TextStyle::new(10.5, 400.0))
                                .color(p.ink_muted),
                        ),
                )
                .child(
                    row()
                        .gap(4.0)
                        .items_center()
                        .child(theme::kbd("2", p))
                        .child(
                            text("Lost it")
                                .font(TextStyle::new(10.5, 400.0))
                                .color(p.ink_muted),
                        ),
                )
                .child(
                    row()
                        .gap(4.0)
                        .items_center()
                        .child(theme::kbd("3", p))
                        .child(
                            text("Don't care")
                                .font(TextStyle::new(10.5, 400.0))
                                .color(p.ink_muted),
                        ),
                ),
        );
    } else {
        out = out.child(
            row()
                .key("canvas-hints")
                .gap(10.0)
                .items_center()
                .child(theme::kbd("Scroll", p))
                .child(
                    text("Zoom")
                        .font(TextStyle::new(10.5, 400.0))
                        .color(p.ink_muted),
                )
                .child(theme::kbd("Ctrl", p))
                .child(
                    text("Fine zoom")
                        .font(TextStyle::new(10.5, 400.0))
                        .color(p.ink_muted),
                )
                .child(theme::kbd("Alt", p))
                .child(
                    text("Move")
                        .font(TextStyle::new(10.5, 400.0))
                        .color(p.ink_muted),
                )
                .child(theme::kbd("Middle-drag", p))
                .child(
                    text("Pan")
                        .font(TextStyle::new(10.5, 400.0))
                        .color(p.ink_muted),
                ),
        );
        out = out.child(
            text(&s.status)
                .font(TextStyle::new(11.0, 400.0))
                .color(p.ink_muted)
                .max_w(320.0)
                .truncate(),
        );
    }

    out = out.child(el().grow(1.0));

    if s.can_page() {
        out = out
            .child(
                text("‹")
                    .key("prev-page")
                    .role("button")
                    .label("Previous page")
                    .focusable(true)
                    .font(TextStyle::new(12.0, 500.0))
                    .color(p.ink_secondary)
                    .px(6.0)
                    .py(2.0)
                    .radius(4.0)
                    .hover(p.hover)
                    .on_press(|s: &mut App| s.turn_page(-1)),
            )
            .child(
                text(format!("{} / {}", s.page, s.page_count()))
                    .font(type_::MONO_CAPTION)
                    .color(p.ink_muted),
            )
            .child(
                text("›")
                    .key("next-page")
                    .role("button")
                    .label("Next page")
                    .focusable(true)
                    .font(TextStyle::new(12.0, 500.0))
                    .color(p.ink_secondary)
                    .px(6.0)
                    .py(2.0)
                    .radius(4.0)
                    .hover(p.hover)
                    .on_press(|s: &mut App| s.turn_page(1)),
            );
    }

    if s.workspace == Workspace::MarkUp {
        out = out
            .child(
                text(format!(
                    "{:.0}×{:.0} · {:.0}%",
                    s.viewport[0],
                    s.viewport[1],
                    s.camera.zoom * 100.0
                ))
                .key("zoom")
                .font(type_::MONO_CAPTION)
                .color(p.ink_muted),
            )
            .child(
                text("Fit")
                    .key("fit")
                    .role("button")
                    .focusable(true)
                    .font(TextStyle::new(10.5, 500.0))
                    .color(p.ink_secondary)
                    .px(6.0)
                    .py(2.0)
                    .radius(4.0)
                    .hover(p.hover)
                    .on_press(|s: &mut App| s.fit()),
            );
    }

    out = out.child(
        row()
            .key("undo")
            .role("button")
            .label("Undo")
            .focusable(true)
            .gap(4.0)
            .items_center()
            .px(6.0)
            .py(2.0)
            .radius(4.0)
            .hover(p.hover)
            .on_press(|s: &mut App| s.undo())
            .child(
                text("Undo")
                    .font(TextStyle::new(10.5, 500.0))
                    .color(p.ink_secondary),
            )
            .child(theme::kbd("Ctrl+Z", p)),
    );

    if !s.message.is_empty() {
        out = out.child(
            text(&s.message)
                .key("message")
                .font(type_::MONO_CAPTION)
                .color(p.ink_secondary)
                .px(7.0)
                .py(2.0)
                .radius(4.0)
                .fill(p.bg_surface)
                .border(1.0, p.border_hair)
                .max_w(240.0)
                .truncate(),
        );
    }

    out
}

fn on_key(s: &mut App, ev: &KeyEvent, tx: &mut Tx) -> bool {
    // A focused text field owns the keyboard: Backspace or shortcut keys must not fire while typing.
    let typing = matches!(
        tx.focused(),
        Some("note" | "label-field" | "target-field" | "pair-note")
    );
    match &ev.key {
        Key::Command('z') | Key::Command('Z') => {
            s.undo();
            true
        }
        Key::Command('o') | Key::Command('O') => {
            files::open(s, tx);
            true
        }
        Key::Command('e') | Key::Command('E') => {
            files::save(s, tx);
            true
        }
        Key::Escape => {
            if matches!(s.mode, app::Mode::Draw { .. } | app::Mode::Select { .. } | app::Mode::Pan { .. }) {
                s.mode = app::Mode::Idle;
                s.status = s.tool.hint().to_string();
            } else {
                s.deselect();
            }
            true
        }
        Key::Delete | Key::Backspace if s.selected.is_some() && !typing => {
            s.delete_selected();
            true
        }
        Key::Text(t) if !ev.mods.cmd && !ev.mods.ctrl && !typing => {
            if s.workspace == Workspace::Calibrate {
                match t.as_str() {
                    "1" => { s.judge_probe(FlinchVerdict::StillHasIt); return true; }
                    "2" => { s.judge_probe(FlinchVerdict::LostIt); return true; }
                    "3" => { s.judge_probe(FlinchVerdict::DontCare); return true; }
                    _ => {}
                }
            }
            let tool = match t.to_ascii_lowercase().as_str() {
                "v" => Some(Tool::Select),
                "h" => Some(Tool::Pan),
                "r" => Some(Tool::Rect),
                "o" => Some(Tool::Ellipse),
                "f" => Some(Tool::Freehand),
                _ => None,
            };
            if let Some(tool) = tool {
                s.workspace = Workspace::MarkUp;
                s.tool = tool;
                s.mode = app::Mode::Idle;
                s.status = tool.hint().to_string();
                return true;
            }
            false
        }
        Key::Command('0') => {
            s.fit();
            true
        }
        _ => false,
    }
}

/// Where the sample project lives: beside the executable in a release zip, or in the repo when
/// running from a checkout.
fn seed_dir() -> Option<std::path::PathBuf> {
    let mut candidates = vec![std::path::PathBuf::from("seed")];
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join("seed"));
        }
    }
    candidates.into_iter().find(|p| p.join("project.json").is_file())
}

fn demo(s: &mut App) {
    if let Some(seed) = seed_dir() {
        let seed = seed.as_path();
        let project = seed.join("project.json");
        if let Ok(text) = std::fs::read_to_string(&project) {
            if let Ok(mut p) = serde_json::from_str::<taste_curator::model::Project>(&text) {
                for artifact in &mut p.artifacts {
                    rebase(&mut artifact.source, seed);
                }
                s.project = p;
                s.target_draft = rune::TextEdit::new(&s.project.target);
                if let Some(id) = s.project.artifacts.first().map(|a| a.id) {
                    s.open_artifact(id);
                }
            }
        }
    }
}

/// The seed project stores paths relative to the web app's public folder.
fn rebase(source: &mut taste_curator::model::ArtifactSource, root: &std::path::Path) {
    use taste_curator::model::ArtifactSource;
    let path = std::path::Path::new(source.path());
    if path.is_absolute() {
        return;
    }
    let name = source.path().trim_start_matches("seed/");
    let full = root.join(name);
    let full = full.display().to_string();
    match source {
        ArtifactSource::Image { path, .. } => *path = full,
        ArtifactSource::Text { path, .. } => *path = full,
        ArtifactSource::Pdf { path, .. } => *path = full,
        ArtifactSource::Html { path, .. } => *path = full,
    }
}

fn empty(s: &mut App) {
    s.project = taste_curator::model::Project::default();
    s.active = None;
    s.selected = None;
    s.pages.clear();
    s.texts.clear();
}

fn with_markings(s: &mut App) {
    demo(s);
    s.workspace = Workspace::MarkUp;
}

fn calibrate_scene(s: &mut App) {
    demo(s);
    s.workspace = Workspace::Calibrate;
    s.refresh_probe();
}

fn scrolled(s: &mut App) {
    demo(s);
    s.workspace = Workspace::MarkUp;
    s.tool = Tool::Rect;
    s.status = Tool::Rect.hint().to_string();
    if let Some(id) = s.project.artifacts.first().and_then(|a| a.annotations.first()).map(|n| n.id) {
        s.select(id);
    }
}

fn main() {
    let mut spec = AppSpec::new("Taste Curator", (1180.0, 760.0), App::default, view);
    spec.min_size = Some((880.0, 560.0));
    spec.scenes = &[
        ("empty", empty),
        ("project", demo),
        ("calibrate", calibrate_scene),
        ("marking", scrolled),
        ("drawn", with_markings),
    ];
    spec.on_key = Some(on_key);
    spec.poll = Some(|s: &mut App, _, _| {
        s.sync_note();
        s.sync_target();
        store::save(s);
        false
    });
    spec.setup = Some(|s: &mut App, _| store::load(s));
    rune::run(spec);
}
