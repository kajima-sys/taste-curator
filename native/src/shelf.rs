//! The left references rail: compact Linear-style rows with active indigo indicator, calibration
//! and marking counts, and a pinned `Import…` action with `Ctrl+O` keycap.

use rune::tok::type_;
use rune::{col, el, row, text, Color, Cx, El, TextStyle};
use taste_curator::calibrate::{self, AxisStatus};
use taste_curator::model::{ArtifactSource, Polarity, ProbeAxis};

use crate::app::App;
use crate::canvas::swatch;
use crate::theme;

pub fn view(s: &App, _cx: &mut Cx<App>) -> El {
    let p = theme::palette(s.dark);
    let mut list = col().key("shelf-list").w_full().gap(2.0);

    if s.project.artifacts.is_empty() {
        list = list.child(
            col()
                .key("shelf-empty")
                .w_full()
                .padding(10.0)
                .radius(6.0)
                .fill(p.bg_surface)
                .border(1.0, p.border_hair)
                .gap(4.0)
                .child(
                    text("No references yet")
                        .font(TextStyle::new(12.0, 500.0))
                        .color(p.ink_primary),
                )
                .child(
                    text("Import an image, PDF, web page, or code snippet to start calibrating.")
                        .font(TextStyle::new(11.0, 400.0).lh(15.0))
                        .color(p.ink_muted)
                        .w_full(),
                ),
        );
    }

    for artifact in &s.project.artifacts {
        let id = artifact.id;
        let active = s.active == Some(id);
        let count = |pol: Polarity| artifact.annotations.iter().filter(|n| n.polarity == pol).count();
        let (likes, avoids, contrasts) = (
            count(Polarity::Like),
            count(Polarity::Dislike),
            count(Polarity::Contrast),
        );
        let locked = ProbeAxis::ALL
            .iter()
            .filter(|&&ax| calibrate::axis_status(&s.project, id, ax) == AxisStatus::HardInvariant)
            .count();
        let free = ProbeAxis::ALL
            .iter()
            .filter(|&&ax| calibrate::axis_status(&s.project, id, ax) == AxisStatus::Tolerance)
            .count();

        let kind = match &artifact.source {
            ArtifactSource::Image { width, height, .. } => format!("{width}×{height}"),
            ArtifactSource::Text { language, .. } => language.clone(),
            ArtifactSource::Html { .. } => "html".to_string(),
            ArtifactSource::Pdf { pages, .. } => format!("{pages}p"),
        };

        let mut meta = row().key(format!("marks-{id}")).gap(6.0).items_center();
        if locked > 0 {
            meta = meta.child(
                row()
                    .gap(3.0)
                    .items_center()
                    .child(theme::dot(5.0, p.flinch))
                    .child(
                        text(format!("{locked}"))
                            .font(TextStyle::new(10.0, 500.0))
                            .color(p.ink_secondary),
                    ),
            );
        }
        if free > 0 {
            meta = meta.child(
                row()
                    .gap(3.0)
                    .items_center()
                    .child(theme::dot(5.0, p.keep))
                    .child(
                        text(format!("{free}"))
                            .font(TextStyle::new(10.0, 500.0))
                            .color(p.ink_secondary),
                    ),
            );
        }
        for (n, pol) in [
            (likes, Polarity::Like),
            (avoids, Polarity::Dislike),
            (contrasts, Polarity::Contrast),
        ] {
            if n > 0 {
                meta = meta.child(
                    row()
                        .gap(2.0)
                        .items_center()
                        .child(el().size(4.5, 4.5).radius(1.5).fill(swatch(pol)))
                        .child(
                            text(format!("{n}"))
                                .font(TextStyle::new(10.0, 500.0))
                                .color(p.ink_muted),
                        ),
                );
            }
        }
        meta = meta.child(
            text(kind)
                .font(type_::MONO_CAPTION)
                .color(p.ink_muted),
        );

        list = list.child(
            row()
                .key(format!("artifact-{id}"))
                .role("button")
                .label(format!("Open {}", artifact.name))
                .focusable(true)
                .w_full()
                .px(8.0)
                .py(7.0)
                .gap(8.0)
                .items_center()
                .radius(6.0)
                .fill(if active { p.bg_elevated } else { Color::TRANSPARENT })
                .border(1.0, if active { p.border_hair } else { Color::TRANSPARENT })
                .hover(p.hover)
                .on_press(move |s: &mut App| s.open_artifact(id))
                .child(
                    el()
                        .size(2.5, 22.0)
                        .radius(2.0)
                        .fill(if active { p.accent } else { Color::TRANSPARENT }),
                )
                .child(
                    col()
                        .grow(1.0)
                        .gap(3.0)
                        .child(
                            text(&artifact.name)
                                .font(TextStyle::new(12.0, if active { 500.0 } else { 400.0 }))
                                .color(if active { p.ink_primary } else { p.ink_secondary })
                                .max_w(162.0)
                                .truncate(),
                        )
                        .child(meta),
                ),
        );
    }

    col()
        .key("shelf")
        .w(216.0)
        .h_full()
        .padding(10.0)
        .gap(10.0)
        .fill(p.bg_rail)
        .border_sides([0.0, 1.0, 0.0, 0.0], p.border_hair)
        .child(
            row()
                .w_full()
                .justify_between()
                .items_center()
                .px(4.0)
                .child(theme::section_label("References", p))
                .child(
                    text(format!("{}", s.project.artifacts.len()))
                        .font(TextStyle::new(10.5, 500.0))
                        .color(p.ink_muted)
                        .px(6.0)
                        .py(1.5)
                        .radius(10.0)
                        .fill(p.bg_surface),
                ),
        )
        .child(list.grow(1.0).scroll(rune::ScrollSpec { stick_bottom: false }))
        .child(theme::divider(p))
        .child(
            row()
                .key("import")
                .role("button")
                .label("Import reference")
                .focusable(true)
                .w_full()
                .justify_between()
                .items_center()
                .px(10.0)
                .py(7.0)
                .radius(6.0)
                .fill(p.bg_surface)
                .border(1.0, p.border_hair)
                .hover(p.hover)
                .on_press_tx(|s: &mut App, tx: &mut rune::Tx| crate::files::open(s, tx))
                .child(
                    text("＋  Import…")
                        .font(TextStyle::new(12.0, 500.0))
                        .color(p.ink_primary),
                )
                .child(theme::kbd("Ctrl+O", p)),
        )
}
