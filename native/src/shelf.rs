//! The shelf: every imported reference, with how many markings it has. Press one to open it.

use rune::tok::{self, radius, space, type_};
use rune::{col, el, row, text, Color, Cx, Drag, El, Phase};
use taste_curator::model::{ArtifactSource, Polarity};

use crate::app::App;
use crate::canvas::swatch;

pub fn view(s: &App, _cx: &mut Cx<App>) -> El {
    let t = tok::theme(s.dark);
    let mut list = col().key("shelf-list").w_full().gap(space::S1);
    if s.project.artifacts.is_empty() {
        list = list.child(
            text("Import a picture, a page or a snippet to start.")
                .font(type_::BODY)
                .color(t.text_secondary)
                .w_full(),
        );
    }
    for artifact in &s.project.artifacts {
        let id = artifact.id;
        let active = s.active == Some(id);
        let count = |p: Polarity| artifact.annotations.iter().filter(|n| n.polarity == p).count();
        let (likes, avoids, contrasts) = (count(Polarity::Like), count(Polarity::Dislike), count(Polarity::Contrast));
        let kind = match &artifact.source {
            ArtifactSource::Image { width, height, .. } => format!("{width}×{height}"),
            ArtifactSource::Text { language, .. } => language.clone(),
            ArtifactSource::Html { .. } => "html".to_string(),
            ArtifactSource::Pdf { pages, .. } => format!("{pages} pages"),
        };
        let mut marks = row().key("marks").gap(6.0).items_center();
        for (n, p) in [(likes, Polarity::Like), (avoids, Polarity::Dislike), (contrasts, Polarity::Contrast)] {
            if n > 0 {
                marks = marks.child(
                    row()
                        .gap(2.0)
                        .items_center()
                        .child(el().size(5.0, 5.0).radius(2.5).fill(swatch(p)))
                        .child(text(format!("{n}")).font(rune::TextStyle::new(10.0, 600.0)).color(t.text_secondary)),
                );
            }
        }
        marks = marks.child(text(kind).font(type_::MONO_CAPTION).color(t.text_secondary));
        list = list.child(
            col()
                .key(format!("artifact-{id}"))
                .role("button")
                .label(format!("Open {}", artifact.name))
                .w_full()
                .padding(space::S2)
                .radius(radius::SM)
                .gap(3.0)
                .fill(if active { t.card } else { Color::TRANSPARENT })
                .hover(t.hover)
                .on_press(move |s: &mut App| s.open_artifact(id))
                .child(
                    text(&artifact.name)
                        .font(type_::LABEL)
                        .color(if active { t.text } else { t.text_secondary })
                        .w_full()
                        .truncate(),
                )
                .child(marks),
        );
    }

    col()
        .key("shelf")
        .w(196.0)
        .h_full()
        .padding(space::S2)
        .gap(space::S2)
        .fill(t.bg)
        .border_sides([0.0, 0.0, 1.0, 0.0], t.separator)
        .child(text("References").font(type_::SECTION).color(t.text_secondary))
        .child(list)
        .child(
            text("Import…")
                .key("import")
                .role("button")
                .focusable(true)
                .font(type_::LABEL)
                .color(t.text)
                .px(space::S2)
                .py(space::S1)
                .radius(radius::SM)
                .fill(t.card)
                .hover(t.hover)
                .on_press_tx(|s: &mut App, tx: &mut rune::Tx| crate::files::open(s, tx)),
        )
}

/// Where a slider's track drag lands, `0..1`.
pub fn slider_value(d: &Drag, start: f32, width: f32) -> f32 {
    let v = if d.phase == Phase::Down { d.x / width } else { start + d.dx / width };
    if v.is_finite() { v.clamp(0.0, 1.0) } else { 0.0 }
}
