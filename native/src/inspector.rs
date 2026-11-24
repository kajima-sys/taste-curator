//! The Mark-up inspector: what the selected marking means, in the curator's own words.
//! Styled with Linear/Raycast hairline surfaces, recessed controls, and quiet pixel facts.

use rune::tok::type_;
use rune::{col, el, row, text, text_input, Color, Cx, El, TextStyle};
use taste_curator::model::Polarity;

use crate::app::App;
use crate::theme::{self, Palette};

pub fn view(s: &App, _cx: &mut Cx<App>) -> El {
    let p = theme::palette(s.dark);
    let mut body = col().key("inspector-body").w_full().gap(14.0);

    let header_badge = match s.selected_annotation() {
        Some((_, n)) => format!("#{:02} · {}", n.id, crate::app::shape_label(n.shape)),
        None => "Mark up".into(),
    };

    match s.selected_annotation() {
        Some((artifact, n)) => {
            let polarity = n.polarity;
            let mine = n.labels.clone();
            let artifact_name = s
                .project
                .artifact(artifact)
                .map(|a| a.name.clone())
                .unwrap_or_default();

            body = body
                .child(
                    text(&artifact_name)
                        .font(type_::MONO_CAPTION)
                        .color(p.ink_muted)
                        .w_full()
                        .truncate(),
                )
                .child(
                    col()
                        .w_full()
                        .gap(6.0)
                        .child(theme::section_label("Reads as", p))
                        .child(polarity_segment(polarity, p)),
                )
                .child(
                    col()
                        .w_full()
                        .gap(6.0)
                        .child(theme::section_label("What you want to remember", p))
                        .child(
                            col()
                                .key("note-field")
                                .w_full()
                                .min_h(96.0)
                                .padding(8.0)
                                .radius(6.0)
                                .fill(p.bg_canvas)
                                .border(1.0, p.border_hair)
                                .child(
                                    text_input(&s.note, TextStyle::new(12.0, 400.0).lh(17.0))
                                        .key("note")
                                        .label("What you want to remember")
                                        .color(p.ink_primary)
                                        .placeholder(
                                            "Anything at all: why it works, what to copy, what to avoid.",
                                            p.ink_muted,
                                        )
                                        .on_edit(|s: &mut App| &mut s.note),
                                ),
                        ),
                )
                .child(label_rows(s, &mine, p))
                .child(theme::divider(p))
                .child(
                    col()
                        .w_full()
                        .gap(6.0)
                        .child(
                            row()
                                .w_full()
                                .justify_between()
                                .items_center()
                                .child(theme::section_label("What the pixels say", p))
                                .child(
                                    text(if s.show_values { "Hide values" } else { "Exact values" })
                                        .key("toggle-values")
                                        .role("button")
                                        .focusable(true)
                                        .font(TextStyle::new(10.5, 500.0))
                                        .color(p.ink_secondary)
                                        .px(6.0)
                                        .py(2.0)
                                        .radius(4.0)
                                        .fill(p.bg_surface)
                                        .border(1.0, p.border_hair)
                                        .hover(p.hover)
                                        .on_press(|s: &mut App| s.show_values = !s.show_values),
                                ),
                        )
                        .child(
                            col()
                                .w_full()
                                .gap(4.0)
                                .children(s.readout.iter().map(|line| {
                                    text(line)
                                        .font(TextStyle::new(11.5, 400.0).lh(16.0))
                                        .color(p.ink_secondary)
                                        .w_full()
                                })),
                        )
                        .child(
                            col()
                                .w_full()
                                .gap(2.0)
                                .children(if s.show_values {
                                    s.exact
                                        .iter()
                                        .map(|line| {
                                            text(line)
                                                .font(type_::MONO_CAPTION)
                                                .color(p.ink_muted)
                                                .w_full()
                                        })
                                        .collect()
                                } else {
                                    Vec::new()
                                }),
                        ),
                )
                .child(
                    row()
                        .w_full()
                        .gap(8.0)
                        .child(
                            text("Delete")
                                .key("delete")
                                .role("button")
                                .focusable(true)
                                .font(TextStyle::new(11.5, 500.0))
                                .color(p.flinch)
                                .px(10.0)
                                .py(6.0)
                                .radius(6.0)
                                .fill(p.bg_surface)
                                .border(1.0, p.border_hair)
                                .hover(p.flinch_soft)
                                .on_press(|s: &mut App| s.delete_selected()),
                        )
                        .child(el().grow(1.0))
                        .child(
                            text("Done")
                                .key("deselect")
                                .role("button")
                                .focusable(true)
                                .font(TextStyle::new(11.5, 500.0))
                                .color(p.ink_primary)
                                .px(12.0)
                                .py(6.0)
                                .radius(6.0)
                                .fill(p.bg_elevated)
                                .border(1.0, p.border_strong)
                                .hover(p.hover)
                                .on_press(|s: &mut App| s.deselect()),
                        ),
                );
        }
        None => {
            body = body
                .child(
                    col()
                        .w_full()
                        .gap(6.0)
                        .padding(12.0)
                        .radius(7.0)
                        .fill(p.bg_surface)
                        .border(1.0, p.border_hair)
                        .child(
                            text("No marking selected")
                                .font(TextStyle::new(12.5, 500.0))
                                .color(p.ink_primary),
                        )
                        .child(
                            text("Draw a box, oval, or loop around a region you care about, then write down what you want to remember.")
                                .font(TextStyle::new(11.5, 400.0).lh(16.0))
                                .color(p.ink_secondary)
                                .w_full(),
                        ),
                )
                .child(
                    col()
                        .w_full()
                        .gap(4.0)
                        .padding(10.0)
                        .radius(6.0)
                        .fill(p.bg_canvas)
                        .border(1.0, p.border_hair)
                        .child(
                            text(&s.status)
                                .font(TextStyle::new(11.5, 500.0))
                                .color(p.ink_primary)
                                .w_full(),
                        )
                        .child(
                            text(&s.message)
                                .font(TextStyle::new(11.0, 400.0))
                                .color(p.ink_muted)
                                .w_full(),
                        ),
                );
        }
    }

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
                    text("Inspector")
                        .font(TextStyle::new(12.5, 600.0))
                        .color(p.ink_primary),
                )
                .child(
                    text(header_badge)
                        .font(type_::MONO_CAPTION)
                        .color(p.ink_muted),
                ),
        )
        .child(theme::divider(p))
        .child(body.grow(1.0).scroll(rune::ScrollSpec { stick_bottom: false }))
}

fn polarity_segment(current: Polarity, p: Palette) -> El {
    let mut well = row()
        .key("choices")
        .w_full()
        .padding(3.0)
        .gap(3.0)
        .radius(7.0)
        .fill(p.bg_canvas)
        .border(1.0, p.border_hair);

    for (i, pol) in Polarity::ALL.iter().enumerate() {
        let on = *pol == current;
        let dot_col = match pol {
            Polarity::Like => p.keep,
            Polarity::Dislike => p.flinch,
            Polarity::Contrast => p.contrast,
        };
        let target = *pol;
        well = well.child(
            row()
                .key(format!("choice-{i}"))
                .role("button")
                .label(pol.label())
                .focusable(true)
                .grow(1.0)
                .center()
                .gap(5.0)
                .py(5.0)
                .px(6.0)
                .radius(5.0)
                .fill(if on { p.bg_elevated } else { Color::TRANSPARENT })
                .border(1.0, if on { p.border_strong } else { Color::TRANSPARENT })
                .hover(p.hover)
                .on_press(move |s: &mut App| s.set_polarity(target))
                .child(theme::dot(5.5, dot_col))
                .child(
                    text(pol.label())
                        .font(TextStyle::new(11.0, if on { 500.0 } else { 400.0 }))
                        .color(if on { p.ink_primary } else { p.ink_secondary }),
                ),
        );
    }
    well
}

/// The labels already used anywhere in the project, plus a field for a new one.
fn label_rows(s: &App, mine: &[String], p: Palette) -> El {
    let mut chips = el().key("label-chips").w_full().gap(5.0).flex_wrap();
    let vocab = s.vocabulary();
    let any = !vocab.is_empty();
    for (word, count) in vocab.into_iter().take(10) {
        let on = mine.iter().any(|l| l.eq_ignore_ascii_case(&word));
        let key = word.clone();
        chips = chips.child(
            text(&word)
                .key(format!("label-{word}"))
                .role("button")
                .label(if count > 1 {
                    format!("{word}, used {count} times")
                } else {
                    word.clone()
                })
                .focusable(true)
                .font(TextStyle::new(11.0, 500.0))
                .color(if on { p.ink_primary } else { p.ink_secondary })
                .px(8.0)
                .py(3.5)
                .radius(5.0)
                .fill(if on { p.accent_soft } else { p.bg_surface })
                .border(1.0, if on { p.accent } else { p.border_hair })
                .hover(p.hover)
                .on_press(move |s: &mut App| s.toggle_label(&key)),
        );
    }
    if !any {
        chips = chips.child(
            text("No labels yet — add one below.")
                .font(TextStyle::new(11.0, 400.0))
                .color(p.ink_muted)
                .w_full(),
        );
    }

    col()
        .key("labels")
        .w_full()
        .gap(6.0)
        .child(theme::section_label("Labels", p))
        .child(chips)
        .child(
            row()
                .w_full()
                .gap(6.0)
                .items_center()
                .child(
                    el()
                        .grow(1.0)
                        .px(8.0)
                        .py(5.0)
                        .radius(6.0)
                        .fill(p.bg_canvas)
                        .border(1.0, p.border_hair)
                        .child(
                            text_input(&s.label_draft, TextStyle::new(11.5, 400.0))
                                .key("label-field")
                                .label("A short label, in your own words")
                                .single_line()
                                .color(p.ink_primary)
                                .placeholder("add a word…", p.ink_muted)
                                .on_edit(|s: &mut App| &mut s.label_draft)
                                .on_submit(|s: &mut App, _| s.commit_label()),
                        ),
                )
                .child(
                    text("＋")
                        .key("add-label")
                        .role("button")
                        .label("Add this label")
                        .focusable(true)
                        .font(TextStyle::new(13.0, 500.0))
                        .color(p.ink_primary)
                        .px(8.0)
                        .py(4.0)
                        .radius(6.0)
                        .fill(p.bg_surface)
                        .border(1.0, p.border_hair)
                        .hover(p.hover)
                        .on_press(|s: &mut App| s.commit_label()),
                ),
        )
}
