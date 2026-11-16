//! Taste Curator, natively: a shelf of references, a canvas you mark up, and an inspector that
//! turns each marking into structured taste. `--snap out.png` renders it headless.

mod app;
mod canvas;
mod files;
mod inspector;
mod shelf;
mod store;

use app::App;
use rune::tok::{self, radius, space, type_};
use rune::{col, el, row, text, AppSpec, Color, Cx, El, Key, KeyEvent, Tx};
use taste_curator::tools::Tool;

fn view(s: &App, cx: &mut Cx<App>) -> El {
    let t = tok::theme(s.dark);
    cx.foreground(t.text);

    let bar = row()
        .key("topbar")
        .w_full()
        .h(48.0)
        .px(space::S3)
        .gap(space::S2)
        .items_center()
        .fill(t.bg)
        .border_sides([1.0, 0.0, 0.0, 0.0], t.separator)
        .child(text("Taste Curator").font(type_::TITLE).color(t.text))
        .child(
            text(&s.project.name)
                .key("project-name")
                .font(type_::BODY)
                .color(t.text_secondary)
                .max_w(220.0)
                .truncate(),
        )
        .child(el().grow(1.0))
        .child(toolbar(s, t))
        .child(files::export_button(s, t));

    let stage = col()
        .key("stage")
        .w_full()
        .h_full()
        .grow(1.0)
        .child(bar)
        .child(
            row()
                .key("body")
                .w_full()
                .grow(1.0)
                .child(shelf::view(s, cx))
                .child(col().key("main").grow(1.0).h_full().child(canvas_pane(s, t)).child(footer(s, t)))
                .child(inspector::view(s, cx)),
        );

    stage
}

fn toolbar(s: &App, t: tok::Theme) -> El {
    let mut out = row().key("tools").gap(4.0).items_center();
    for tool in Tool::ALL {
        let on = s.tool == tool;
        out = out.child(
            text(tool.label())
                .key(format!("tool-{}", tool.label().to_lowercase()))
                .role("button")
                .label(format!("{} tool", tool.label()))
                .focusable(true)
                .font(type_::LABEL)
                .color(if on { Color::BLACK } else { t.text_secondary })
                .px(8.0)
                .py(3.0)
                .radius(radius::SM)
                .fill(if on { t.knob } else { Color::TRANSPARENT })
                .hover(t.hover)
                .on_press(move |s: &mut App| {
                    s.tool = tool;
                    s.mode = app::Mode::Idle;
                    s.status = tool.hint().to_string();
                }),
        );
    }
    out = out.child(
        text(if s.dark { "◐" } else { "◑" })
            .key("theme-toggle")
            .role("button")
            .font(type_::BODY)
            .color(t.text_secondary)
            .px(6.0)
            .radius(radius::SM)
            .hover(t.hover)
            .on_press(|s: &mut App| s.dark = !s.dark),
    );
    out
}

fn canvas_pane(s: &App, t: tok::Theme) -> El {
    let mut pane = col()
        .key("canvas-pane")
        .w_full()
        .grow(1.0)
        .padding(space::S2)
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
                        .font(type_::BODY)
                        .color(t.text_secondary),
                ),
        );
    }
    pane
}

fn footer(s: &App, t: tok::Theme) -> El {
    let mut out = row()
        .key("footer")
        .w_full()
        .h(34.0)
        .px(space::S3)
        .gap(space::S2)
        .items_center()
        .fill(t.bg)
        .border_sides([1.0, 0.0, 0.0, 0.0], t.separator)
        .child(text(&s.status).font(type_::BODY).color(t.text_secondary).max_w(320.0).truncate())
        .child(el().grow(1.0));
    if s.can_page() {
        out = out
            .child(
                text("‹")
                    .key("prev-page")
                    .role("button")
                    .font(type_::BODY)
                    .color(t.text_secondary)
                    .px(6.0)
                    .hover(t.hover)
                    .on_press(|s: &mut App| s.turn_page(-1)),
            )
            .child(text(format!("{} / {}", s.page, s.page_count())).font(type_::MONO_CAPTION).color(t.text_secondary))
            .child(
                text("›")
                    .key("next-page")
                    .role("button")
                    .font(type_::BODY)
                    .color(t.text_secondary)
                    .px(6.0)
                    .hover(t.hover)
                    .on_press(|s: &mut App| s.turn_page(1)),
            );
    }
    out = out
        .child(
            text(format!("{:.0}x{:.0} {:.0}%", s.viewport[0], s.viewport[1], s.camera.zoom * 100.0))
                .key("zoom")
                .font(type_::MONO_CAPTION)
                .color(t.text_secondary),
        )
        .child(
            text("Fit")
                .key("fit")
                .role("button")
                .font(type_::LABEL)
                .color(t.text_secondary)
                .px(6.0)
                .py(2.0)
                .radius(radius::SM)
                .hover(t.hover)
                .on_press(|s: &mut App| s.fit()),
        )
        .child(
            text("Undo")
                .key("undo")
                .role("button")
                .font(type_::LABEL)
                .color(t.text_secondary)
                .px(6.0)
                .py(2.0)
                .radius(radius::SM)
                .hover(t.hover)
                .on_press(|s: &mut App| s.undo()),
        )
        .child(
            text(&s.message)
                .key("message")
                .font(type_::MONO_CAPTION)
                .color(t.text_secondary)
                .max_w(280.0)
                .truncate(),
        );
    out
}

fn on_key(s: &mut App, ev: &KeyEvent, tx: &mut Tx) -> bool {
    // Never steal keys from a focused field: the note has to be typeable.
    if let Key::Text(_) = &ev.key {
        if !ev.mods.cmd && !ev.mods.ctrl && s.note_focused() {
            return false;
        }
    }
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
            if matches!(s.mode, app::Mode::Draw { .. }) {
                s.mode = app::Mode::Idle;
            } else {
                s.deselect();
            }
            true
        }
        Key::Delete | Key::Backspace if s.selected.is_some() && !s.note_focused() => {
            s.delete_selected();
            true
        }
        Key::Text(t) if !ev.mods.cmd && !ev.mods.ctrl && !s.note_focused() => {
            let tool = match t.to_ascii_lowercase().as_str() {
                "v" => Some(Tool::Select),
                "h" => Some(Tool::Pan),
                "r" => Some(Tool::Rect),
                "o" => Some(Tool::Ellipse),
                "f" => Some(Tool::Freehand),
                _ => None,
            };
            if let Some(tool) = tool {
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

impl App {
    fn note_focused(&self) -> bool {
        // The note field owns the keyboard while it holds focus; the app must not eat letters.
        // Rune routes unhandled keys here only when no control took them, so `true` would block
        // typing. Treat a selected marking with a note field as potentially typing.
        false
    }
}

/// Where the sample project lives: beside the executable in a release zip, or in the repo when
/// running from a checkout.
fn seed_dir() -> Option<std::path::PathBuf> {
    let mut candidates = vec![std::path::PathBuf::from("app/public/seed")];
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join("seed"));
            candidates.push(dir.join("app/public/seed"));
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
}

fn scrolled(s: &mut App) {
    demo(s);
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
        ("marking", scrolled),
        ("drawn", with_markings),
    ];
    spec.on_key = Some(on_key);
    spec.poll = Some(|s: &mut App, _, _| {
        s.sync_note();
        store::save(s);
        false
    });
    spec.setup = Some(|s: &mut App, _| store::load(s));
    rune::run(spec);
}
