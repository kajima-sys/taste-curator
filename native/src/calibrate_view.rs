//! The Counterfactual Calibration studio ("Perturb Until They Flinch") and the live Empirical
//! Acceptance Envelope inspector, styled with Linear/Raycast hairline surfaces and keycaps.

use rune::tok::type_;
use rune::{col, el, row, text, text_input, Color, Cx, El, TextStyle};
use taste_curator::calibrate::{self, AxisStatus};
use taste_curator::model::{FlinchVerdict, ProbeAxis, ProbeDirection};

use crate::app::App;
use crate::theme::{self, Palette};

/// Center workspace in Calibrate mode: side-by-side Original vs Perturbed counterfactual cards,
/// Raycast-style verdict command buttons (`1`, `2`, `3`), 6-axis strip, and "This, not that" foil
/// picker.
pub fn stage_view(s: &App, _cx: &mut Cx<App>) -> El {
    let p = theme::palette(s.dark);
    let Some(artifact_id) = s.active else {
        return col()
            .key("calibrate-empty")
            .w_full()
            .grow(1.0)
            .center()
            .padding(24.0)
            .fill(p.bg_canvas)
            .child(
                col()
                    .padding(20.0)
                    .gap(6.0)
                    .radius(8.0)
                    .fill(p.bg_rail)
                    .border(1.0, p.border_hair)
                    .items_center()
                    .child(
                        text("Import a reference to start calibrating")
                            .font(TextStyle::new(14.0, 500.0))
                            .color(p.ink_primary),
                    )
                    .child(
                        text("We perturb one visual dimension at a time and record where you flinch.")
                            .font(TextStyle::new(12.0, 400.0))
                            .color(p.ink_secondary),
                    ),
            );
    };

    let artifact_name = s
        .project
        .artifact(artifact_id)
        .map(|a| a.name.clone())
        .unwrap_or_default();

    let judged_count = s
        .project
        .probes
        .iter()
        .filter(|pr| pr.artifact == artifact_id)
        .count();

    let mut content = col()
        .key("calibrate-stage")
        .w_full()
        .grow(1.0)
        .padding(14.0)
        .gap(12.0)
        .fill(p.bg_canvas);

    // Header row: clear question, reference badge, Reset button.
    content = content.child(
        row()
            .key("calibrate-header")
            .w_full()
            .justify_between()
            .items_center()
            .child(
                col()
                    .gap(3.0)
                    .child(
                        text("If we change it like this, does it still have what you want?")
                            .font(TextStyle::new(15.0, 600.0))
                            .color(p.ink_primary),
                    )
                    .child(
                        row()
                            .gap(8.0)
                            .items_center()
                            .child(
                                text(&artifact_name)
                                    .font(type_::MONO_CAPTION)
                                    .color(p.ink_secondary),
                            )
                            .child(
                                text(format!(
                                    "{judged_count} judgment{}",
                                    if judged_count == 1 { "" } else { "s" }
                                ))
                                .font(TextStyle::new(10.5, 500.0))
                                .color(p.ink_muted)
                                .px(6.0)
                                .py(1.5)
                                .radius(4.0)
                                .fill(p.bg_surface)
                                .border(1.0, p.border_hair),
                            ),
                    ),
            )
            .child(
                text("Reset")
                    .key("reset-calibration")
                    .role("button")
                    .label("Reset calibration for this reference")
                    .focusable(true)
                    .font(TextStyle::new(11.5, 500.0))
                    .color(p.ink_secondary)
                    .px(10.0)
                    .py(5.0)
                    .radius(6.0)
                    .fill(p.bg_surface)
                    .border(1.0, p.border_hair)
                    .hover(p.hover)
                    .on_press(|s: &mut App| s.reset_calibration()),
            ),
    );

    // Side-by-side counterfactual stage or completion card.
    if let Some(preview) = &s.probe_preview {
        let axis_name = preview.axis.label();
        let change = preview.axis.change_label(preview.direction);
        let aspect = if preview.height > 0 {
            preview.width as f32 / preview.height as f32
        } else {
            1.33
        };
        let max_w = 244.0f32;
        let max_h = 210.0f32;
        let (fw, fh) = if aspect >= max_w / max_h {
            (max_w, (max_w / aspect).max(80.0))
        } else {
            ((max_h * aspect).max(80.0), max_h)
        };

        content = content
            .child(
                row()
                    .key("probe-pair")
                    .w_full()
                    .gap(12.0)
                    .items_center()
                    .child(
                        col()
                            .key("probe-anchor-col")
                            .grow(1.0)
                            .gap(10.0)
                            .padding(12.0)
                            .radius(8.0)
                            .fill(p.bg_rail)
                            .border(1.0, p.border_hair)
                            .items_center()
                            .child(
                                row()
                                    .w_full()
                                    .justify_between()
                                    .items_center()
                                    .child(
                                        text("Original")
                                            .font(TextStyle::new(11.5, 500.0))
                                            .color(p.ink_secondary),
                                    )
                                    .child(
                                        text("Anchor")
                                            .font(TextStyle::new(10.0, 500.0))
                                            .color(p.ink_muted)
                                            .px(6.0)
                                            .py(1.5)
                                            .radius(4.0)
                                            .fill(p.bg_surface),
                                    ),
                            )
                            .child(
                                el()
                                    .key("probe-anchor-frame")
                                    .padding(6.0)
                                    .radius(6.0)
                                    .fill(p.bg_canvas)
                                    .border(1.0, p.border_hair)
                                    .child(
                                        el()
                                            .key("probe-anchor-img")
                                            .size(fw, fh)
                                            .pixels(&preview.anchor),
                                    ),
                            ),
                    )
                    .child(
                        col()
                            .key("probe-perturbed-col")
                            .grow(1.0)
                            .gap(10.0)
                            .padding(12.0)
                            .radius(8.0)
                            .fill(p.bg_rail)
                            .border(1.0, p.border_strong)
                            .items_center()
                            .child(
                                row()
                                    .w_full()
                                    .justify_between()
                                    .items_center()
                                    .child(
                                        row()
                                            .gap(6.0)
                                            .items_center()
                                            .child(theme::dot(6.0, p.accent))
                                            .child(
                                                text(change)
                                                    .key("probe-change-label")
                                                    .font(TextStyle::new(11.5, 600.0))
                                                    .color(p.ink_primary),
                                            ),
                                    )
                                    .child(
                                        text(axis_name)
                                            .font(TextStyle::new(10.0, 500.0))
                                            .color(p.accent)
                                            .px(6.0)
                                            .py(1.5)
                                            .radius(4.0)
                                            .fill(p.accent_soft),
                                    ),
                            )
                            .child(
                                el()
                                    .key("probe-perturbed-frame")
                                    .padding(6.0)
                                    .radius(6.0)
                                    .fill(p.bg_canvas)
                                    .border(1.0, p.border_hair)
                                    .child(
                                        el()
                                            .key("probe-perturbed-img")
                                            .size(fw, fh)
                                            .pixels(&preview.perturbed),
                                    ),
                            ),
                    ),
            )
            .child(
                row()
                    .key("verdict-bar")
                    .w_full()
                    .gap(8.0)
                    .items_center()
                    .child(verdict_button(
                        "verdict-still",
                        "Still has it",
                        "1",
                        p.keep,
                        p.keep_soft,
                        p,
                        FlinchVerdict::StillHasIt,
                    ))
                    .child(verdict_button(
                        "verdict-lost",
                        "Lost it",
                        "2",
                        p.flinch,
                        p.flinch_soft,
                        p,
                        FlinchVerdict::LostIt,
                    ))
                    .child(verdict_button(
                        "verdict-dont-care",
                        "Don't care",
                        "3",
                        p.ink_muted,
                        p.bg_surface,
                        p,
                        FlinchVerdict::DontCare,
                    )),
            );
    } else if s.pages.contains_key(&(artifact_id, s.page)) {
        content = content.child(
            col()
                .key("calibration-done")
                .w_full()
                .padding(16.0)
                .gap(6.0)
                .radius(8.0)
                .fill(p.bg_rail)
                .border(1.0, p.border_hair)
                .child(
                    row()
                        .gap(8.0)
                        .items_center()
                        .child(theme::dot(7.0, p.keep))
                        .child(
                            text("All six axes calibrated for this reference")
                                .font(TextStyle::new(13.5, 600.0))
                                .color(p.ink_primary),
                        ),
                )
                .child(
                    text("Click any axis below to re-test it, pick another reference on the left, or export the acceptance envelope as PDF.")
                        .font(TextStyle::new(12.0, 400.0).lh(17.0))
                        .color(p.ink_secondary)
                        .w_full(),
                ),
        );
    } else {
        content = content.child(
            col()
                .key("calibration-no-pixels")
                .w_full()
                .padding(16.0)
                .radius(8.0)
                .fill(p.bg_rail)
                .border(1.0, p.border_hair)
                .child(
                    text("This reference is plain text — visual perturbation runs on images, PDF pages, and HTML pages. Switch to Mark up to annotate text.")
                        .font(TextStyle::new(12.0, 400.0).lh(17.0))
                        .color(p.ink_secondary)
                        .w_full(),
                ),
        );
    }

    // Six perceptual axes strip.
    let mut axis_row = el().key("axis-strip").w_full().gap(6.0).flex_wrap();
    for axis in ProbeAxis::ALL {
        let status = calibrate::axis_status(&s.project, artifact_id, axis);
        let active = s.active_probe.is_some_and(|(a, _, _)| a == axis);
        let dot_color = match status {
            AxisStatus::HardInvariant => p.flinch,
            AxisStatus::Tolerance => p.keep,
            AxisStatus::Partial => p.accent,
            AxisStatus::Untested => p.border_strong,
        };
        let status_word = match status {
            AxisStatus::HardInvariant => "locked",
            AxisStatus::Tolerance => "free",
            AxisStatus::Partial => "1/2",
            AxisStatus::Untested => "untested",
        };
        axis_row = axis_row.child(
            row()
                .key(format!("axis-{}", axis.slug()))
                .role("button")
                .label(format!("{}: {status_word}", axis.label()))
                .focusable(true)
                .gap(6.0)
                .items_center()
                .px(9.0)
                .py(5.0)
                .radius(6.0)
                .fill(if active { p.bg_elevated } else { p.bg_rail })
                .border(1.0, if active { p.border_strong } else { p.border_hair })
                .hover(p.hover)
                .on_press(move |s: &mut App| {
                    let dir = match s.active_probe {
                        Some((a, d, _)) if a == axis => d.opposite(),
                        _ => ProbeDirection::Negative,
                    };
                    s.select_probe(axis, dir);
                })
                .child(theme::dot(6.0, dot_color))
                .child(
                    text(axis.label())
                        .font(TextStyle::new(11.5, if active { 500.0 } else { 400.0 }))
                        .color(if active { p.ink_primary } else { p.ink_secondary }),
                ),
        );
    }
    content = content.child(
        col()
            .key("axes-block")
            .w_full()
            .gap(6.0)
            .child(
                row()
                    .w_full()
                    .justify_between()
                    .items_center()
                    .child(theme::section_label("Axes", p))
                    .child(
                        text("Click an axis to test or flip direction")
                            .font(TextStyle::new(10.5, 400.0))
                            .color(p.ink_muted),
                    ),
            )
            .child(axis_row),
    );

    // Contrastive pair strip ("This, not that").
    let others: Vec<(u32, String)> = s
        .project
        .artifacts
        .iter()
        .filter(|a| a.id != artifact_id)
        .map(|a| (a.id, a.name.clone()))
        .collect();
    if !others.is_empty() {
        let mut foils = el().key("foil-choices").w_full().gap(6.0).flex_wrap();
        for (oid, oname) in others {
            foils = foils.child(
                row()
                    .key(format!("pair-foil-{oid}"))
                    .role("button")
                    .label(format!("Pair {artifact_name} against {oname}"))
                    .focusable(true)
                    .gap(5.0)
                    .items_center()
                    .px(9.0)
                    .py(4.5)
                    .radius(6.0)
                    .fill(p.bg_surface)
                    .border(1.0, p.border_hair)
                    .hover(p.hover)
                    .on_press(move |s: &mut App| s.commit_pair(oid))
                    .child(
                        text("not")
                            .font(TextStyle::new(10.5, 500.0))
                            .color(p.ink_muted),
                    )
                    .child(
                        text(oname)
                            .font(TextStyle::new(11.5, 500.0))
                            .color(p.ink_primary),
                    ),
            );
        }
        content = content.child(
            col()
                .key("contrastive-strip")
                .w_full()
                .gap(8.0)
                .padding(12.0)
                .radius(8.0)
                .fill(p.bg_rail)
                .border(1.0, p.border_hair)
                .child(
                    row()
                        .w_full()
                        .justify_between()
                        .items_center()
                        .child(theme::section_label("This, not that", p))
                        .child(
                            text(format!("Anchor: {artifact_name}"))
                                .font(type_::MONO_CAPTION)
                                .color(p.ink_muted),
                        ),
                )
                .child(
                    el()
                        .w_full()
                        .px(8.0)
                        .py(6.0)
                        .radius(6.0)
                        .fill(p.bg_canvas)
                        .border(1.0, p.border_hair)
                        .child(
                            text_input(&s.pair_note, TextStyle::new(12.0, 400.0))
                                .key("pair-note")
                                .label("What breaks when turning this reference into the foil")
                                .single_line()
                                .color(p.ink_primary)
                                .placeholder(
                                    "Optional note: what breaks when turning this into the foil?",
                                    p.ink_muted,
                                )
                                .on_edit(|s: &mut App| &mut s.pair_note),
                        ),
                )
                .child(foils),
        );
    }

    content.scroll(rune::ScrollSpec { stick_bottom: false })
}

fn verdict_button(
    key: &'static str,
    label: &'static str,
    shortcut: &'static str,
    dot_color: Color,
    hover_tint: Color,
    p: Palette,
    verdict: FlinchVerdict,
) -> El {
    row()
        .key(key)
        .role("button")
        .label(format!("{label} ({shortcut})"))
        .focusable(true)
        .grow(1.0)
        .justify_between()
        .items_center()
        .px(12.0)
        .py(9.0)
        .radius(7.0)
        .fill(p.bg_surface)
        .border(1.0, p.border_strong)
        .hover(hover_tint)
        .on_press(move |s: &mut App| s.judge_probe(verdict))
        .child(
            row()
                .gap(8.0)
                .items_center()
                .child(theme::dot(7.0, dot_color))
                .child(
                    text(label)
                        .font(TextStyle::new(12.5, 500.0))
                        .color(p.ink_primary),
                ),
        )
        .child(theme::kbd(shortcut, p))
}

/// Right inspector in Calibrate mode: live Empirical Acceptance Envelope that exports to PDF.
pub fn envelope_inspector(s: &App, _cx: &mut Cx<App>) -> El {
    let p = theme::palette(s.dark);
    let envelope = calibrate::build_envelope(&s.project, &|id, page| {
        s.pages.get(&(id, page.unwrap_or(1))).map(|pg| pg.bitmap.clone())
    });

    let mut body = col().key("envelope-body").w_full().gap(14.0);

    // Target context ("What are you building?").
    body = body.child(
        col()
            .key("target-section")
            .w_full()
            .gap(6.0)
            .child(theme::section_label("What are you building?", p))
            .child(
                el()
                    .key("target-box")
                    .w_full()
                    .px(8.0)
                    .py(6.0)
                    .radius(6.0)
                    .fill(p.bg_canvas)
                    .border(1.0, p.border_hair)
                    .child(
                        text_input(&s.target_draft, TextStyle::new(12.0, 400.0))
                            .key("target-field")
                            .label("What are you building")
                            .single_line()
                            .color(p.ink_primary)
                            .placeholder("e.g. personal reading list page", p.ink_muted)
                            .on_edit(|s: &mut App| &mut s.target_draft),
                    ),
            ),
    );

    body = body.child(theme::divider(p));

    // Hard invariants (flinched).
    let mut inv_col = col().key("hard-invariants").w_full().gap(6.0);
    inv_col = inv_col.child(
        row()
            .w_full()
            .justify_between()
            .items_center()
            .child(theme::section_label("Hard invariants", p))
            .child(
                text(format!("{}", envelope.hard_invariants.len()))
                    .font(TextStyle::new(10.5, 500.0))
                    .color(if envelope.hard_invariants.is_empty() { p.ink_muted } else { p.flinch })
                    .px(6.0)
                    .py(1.0)
                    .radius(10.0)
                    .fill(if envelope.hard_invariants.is_empty() { p.bg_surface } else { p.flinch_soft }),
            ),
    );
    if envelope.hard_invariants.is_empty() {
        inv_col = inv_col.child(
            text("None yet — press Lost it when a perturbation breaks what you like.")
                .font(TextStyle::new(11.5, 400.0).lh(16.0))
                .color(p.ink_muted)
                .w_full(),
        );
    } else {
        for (i, inv) in envelope.hard_invariants.iter().enumerate() {
            inv_col = inv_col.child(
                col()
                    .key(format!("inv-{i}"))
                    .w_full()
                    .gap(3.0)
                    .px(10.0)
                    .py(8.0)
                    .radius(6.0)
                    .fill(p.bg_surface)
                    .border(1.0, p.border_hair)
                    .child(
                        row()
                            .w_full()
                            .gap(6.0)
                            .items_center()
                            .child(theme::dot(6.0, p.flinch))
                            .child(
                                text(format!("{} · {}", inv.axis.label(), inv.artifact_name))
                                    .font(TextStyle::new(11.5, 500.0))
                                    .color(p.ink_primary)
                                    .max_w(225.0)
                                    .truncate(),
                            ),
                    )
                    .child(
                        text(format!("Broke on: {}", inv.flinch_boundaries.join(", ")))
                            .font(type_::MONO_CAPTION)
                            .color(p.ink_secondary)
                            .w_full(),
                    ),
            );
        }
    }
    body = body.child(inv_col);

    // Verified tolerances (free variables).
    let mut tol_col = col().key("verified-tolerances").w_full().gap(6.0);
    tol_col = tol_col.child(
        row()
            .w_full()
            .justify_between()
            .items_center()
            .child(theme::section_label("Free to adapt", p))
            .child(
                text(format!("{}", envelope.verified_tolerances.len()))
                    .font(TextStyle::new(10.5, 500.0))
                    .color(if envelope.verified_tolerances.is_empty() { p.ink_muted } else { p.keep })
                    .px(6.0)
                    .py(1.0)
                    .radius(10.0)
                    .fill(if envelope.verified_tolerances.is_empty() { p.bg_surface } else { p.keep_soft }),
            ),
    );
    if envelope.verified_tolerances.is_empty() {
        tol_col = tol_col.child(
            text("Axes you accept or mark Don't care appear here so the agent won't overfit.")
                .font(TextStyle::new(11.5, 400.0).lh(16.0))
                .color(p.ink_muted)
                .w_full(),
        );
    } else {
        for (i, tol) in envelope.verified_tolerances.iter().enumerate() {
            tol_col = tol_col.child(
                row()
                    .key(format!("tol-{i}"))
                    .w_full()
                    .gap(7.0)
                    .items_center()
                    .px(8.0)
                    .py(5.0)
                    .radius(5.0)
                    .fill(p.bg_surface)
                    .border(1.0, p.border_hair)
                    .child(theme::dot(6.0, p.keep))
                    .child(
                        text(format!("{} · {}", tol.axis.label(), tol.artifact_name))
                            .font(TextStyle::new(11.5, 400.0))
                            .color(p.ink_secondary)
                            .max_w(225.0)
                            .truncate(),
                    ),
            );
        }
    }
    body = body.child(tol_col);

    // Contrastive pairs ("This, not that").
    let mut pair_col = col().key("contrastive-pairs").w_full().gap(6.0);
    pair_col = pair_col.child(
        row()
            .w_full()
            .justify_between()
            .items_center()
            .child(theme::section_label("This, not that", p))
            .child(
                text(format!("{}", envelope.contrastive_pairs.len()))
                    .font(TextStyle::new(10.5, 500.0))
                    .color(p.ink_muted)
                    .px(6.0)
                    .py(1.0)
                    .radius(10.0)
                    .fill(p.bg_surface),
            ),
    );
    if envelope.contrastive_pairs.is_empty() {
        pair_col = pair_col.child(
            text("No contrastive pairs yet.")
                .font(TextStyle::new(11.5, 400.0))
                .color(p.ink_muted)
                .w_full(),
        );
    } else {
        for pair in &envelope.contrastive_pairs {
            let pid = pair.id;
            pair_col = pair_col.child(
                col()
                    .key(format!("pair-card-{pid}"))
                    .w_full()
                    .gap(4.0)
                    .px(10.0)
                    .py(8.0)
                    .radius(6.0)
                    .fill(p.bg_surface)
                    .border(1.0, p.border_hair)
                    .child(
                        row()
                            .w_full()
                            .justify_between()
                            .items_center()
                            .child(
                                text(format!("{} > {}", pair.anchor_name, pair.foil_name))
                                    .font(TextStyle::new(11.5, 500.0))
                                    .color(p.ink_primary)
                                    .max_w(210.0)
                                    .truncate(),
                            )
                            .child(
                                text("×")
                                    .key(format!("remove-pair-{pid}"))
                                    .role("button")
                                    .label("Remove pair")
                                    .focusable(true)
                                    .font(TextStyle::new(12.0, 500.0))
                                    .color(p.ink_muted)
                                    .px(5.0)
                                    .py(1.0)
                                    .radius(4.0)
                                    .hover(p.hover)
                                    .on_press(move |s: &mut App| s.remove_pair(pid)),
                            ),
                    )
                    .child(
                        text(if pair.note.trim().is_empty() {
                            pair.measured_delta.clone()
                        } else {
                            pair.note.clone()
                        })
                        .font(TextStyle::new(11.0, 400.0).lh(15.0))
                        .color(p.ink_secondary)
                        .w_full(),
                    ),
            );
        }
    }
    body = body.child(pair_col);

    col()
        .key("inspector")
        .w(292.0)
        .h_full()
        .padding(12.0)
        .gap(12.0)
        .fill(p.bg_rail)
        .border_sides([0.0, 0.0, 0.0, 1.0], p.border_hair)
        .child(
            row()
                .w_full()
                .justify_between()
                .items_center()
                .child(
                    text("Acceptance envelope")
                        .font(TextStyle::new(12.5, 600.0))
                        .color(p.ink_primary),
                )
                .child(
                    text(format!(
                        "{} locked · {} free",
                        envelope.hard_invariants.len(),
                        envelope.verified_tolerances.len()
                    ))
                    .font(type_::MONO_CAPTION)
                    .color(p.ink_muted),
                ),
        )
        .child(theme::divider(p))
        .child(body.grow(1.0).scroll(rune::ScrollSpec { stick_bottom: false }))
}
