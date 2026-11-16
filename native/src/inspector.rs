//! The inspector: what the selected marking means, in plain words. Polarity, what part of the
//! artifact it is about, where the trait should show up next, the four dials and a note.

use rune::tok::{self, radius, space, type_};
use rune::{col, el, row, text, text_input, Color, Cx, Drag, El, Phase, TextStyle};
use taste_curator::model::{Medium, Polarity, Scope, Spectrum};

use crate::app::App;
use crate::shelf::slider_value;

pub fn view(s: &App, _cx: &mut Cx<App>) -> El {
    let t = tok::theme(s.dark);
    let mut body = col().key("inspector-body").w_full().gap(space::S3);
    match s.selected_annotation() {
        Some((artifact, n)) => {
            let (id, polarity, scope, medium) = (n.id, n.polarity, n.scope, n.medium);
            let chips: Vec<(Spectrum, f32)> = n.chips.clone();
            let artifact_name = s.project.artifact(artifact).map(|a| a.name.clone()).unwrap_or_default();
            body = body
                .child(
                    col()
                        .w_full()
                        .gap(2.0)
                        .child(text(format!("Marking {} · {}", format!("{:02}", id), crate::app::shape_label(n.shape))).font(type_::SECTION).color(t.text_secondary))
                        .child(text(&artifact_name).font(type_::MONO_CAPTION).color(t.text_secondary).w_full().truncate()),
                )
                .child(heading("Reads as", t.text_secondary))
                .child(choices(
                    Polarity::ALL.iter().map(|p| (p.label().to_string(), *p == polarity)).collect(),
                    |i, s: &mut App| s.set_polarity(Polarity::ALL[i]),
                    t,
                ))
                .child(heading("About", t.text_secondary))
                .child(choices(
                    Scope::ALL.iter().map(|p| (p.label().to_string(), *p == scope)).collect(),
                    |i, s: &mut App| s.set_scope(Scope::ALL[i]),
                    t,
                ))
                .child(heading("Next in", t.text_secondary))
                .child(choices(
                    Medium::ALL.iter().map(|p| (p.label().to_string(), *p == medium)).collect(),
                    |i, s: &mut App| s.set_medium(Medium::ALL[i]),
                    t,
                ))
                .child(heading("The feel", t.text_secondary));

            for dial in Spectrum::ALL {
                let value = chips.iter().find(|(d, _)| *d == dial).map(|(_, v)| *v);
                body = body.child(dialer(s, dial, value, t));
            }

            body = body
                .child(
                    col()
                        .key("note-field")
                        .w_full()
                        .min_h(58.0)
                        .padding(space::S2)
                        .radius(radius::MD)
                        .fill(t.card)
                        .border(1.0, t.card_border)
                        .on_edit(|s: &mut App| &mut s.note)
                        .child(
                            text_input(&s.note, type_::BODY.lh(18.0))
                                .key("note")
                                .color(t.text)
                                .placeholder("Why this? Say it in your own words.", t.text_secondary)
                                .on_edit(|s: &mut App| &mut s.note),
                        ),
                )
                .child(
                    col()
                        .w_full()
                        .gap(space::S1)
                        .child(text("Measured, not estimated").font(type_::SECTION).color(t.text_secondary))
                        .child(
                            col()
                                .w_full()
                                .gap(2.0)
                                .children(s.readout.iter().map(|line| {
                                    text(line).font(type_::MONO_CAPTION).color(t.text_secondary).w_full()
                                })),
                        ),
                )
                .child(
                    row()
                        .gap(space::S2)
                        .child(button("delete", "Delete", t, |s: &mut App| s.delete_selected()))
                        .child(button("deselect", "Done", t, |s: &mut App| s.deselect())),
                );
        }
        None => {
            body = body
                .child(heading("Nothing selected", t.text_secondary))
                .child(
                    text("Draw a box, an oval or a loop around the part you care about. Then say what it is about and how it should feel.")
                        .font(type_::BODY)
                        .color(t.text_secondary)
                        .w_full(),
                )
                .child(
                    col()
                        .w_full()
                        .gap(space::S2)
                        .padding(space::S2)
                        .radius(radius::MD)
                        .fill(t.card)
                        .child(text(&s.status).font(type_::BODY).color(t.text).w_full())
                        .child(text(&s.message).font(type_::BODY).color(t.text_secondary).w_full()),
                );
        }
    }

    col()
        .key("inspector")
        .w(272.0)
        .h_full()
        .padding(space::S2)
        .gap(space::S2)
        .fill(t.bg)
        .border_sides([0.0, 0.0, 0.0, 1.0], t.separator)
        .child(
            row()
                .w_full()
                .justify_between()
                .items_center()
                .child(text("Inspector").font(type_::SECTION).color(t.text_secondary))
                .child(
                    text(if s.dark { "Light" } else { "Dark" })
                        .key("theme")
                        .role("button")
                        .font(type_::LABEL)
                        .color(t.text_secondary)
                        .px(6.0)
                        .py(2.0)
                        .radius(radius::SM)
                        .hover(t.hover)
                        .on_press(|s: &mut App| s.dark = !s.dark),
                ),
        )
        .child(body.scroll(rune::ScrollSpec { stick_bottom: false }))
}

fn heading(label: &str, color: Color) -> El {
    text(label).font(type_::SECTION).color(color)
}

fn choices(labels: Vec<(String, bool)>, pick: fn(usize, &mut App), t: tok::Theme) -> El {
    let mut choices_row = el().key("choices").w_full().gap(4.0).flex_wrap();
    let mut args: Vec<(String, bool)> = labels;
    for (i, (label, on)) in args.drain(..).enumerate() {
        choices_row = choices_row.child(
            text(label)
                .key("choice")
                .role("button")
                .font(TextStyle::new(11.5, 500.0))
                .color(if on { Color::BLACK } else { t.text_secondary })
                .px(7.0)
                .py(3.0)
                .radius(radius::SM)
                .fill(if on { t.knob } else { t.card })
                .hover(t.hover)
                .on_press(move |s: &mut App| pick(i, s)),
        );
    }
    choices_row
}

fn button(key: &'static str, label: &'static str, t: tok::Theme, f: fn(&mut App)) -> El {
    text(label)
        .key(key)
        .role("button")
        .focusable(true)
        .font(type_::LABEL)
        .color(t.text)
        .px(space::S2)
        .py(space::S1)
        .radius(radius::SM)
        .fill(t.card)
        .hover(t.hover)
        .on_press(f)
}

/// One perceptual dial. A press anywhere on the track jumps there; dragging keeps adjusting.
fn dialer(s: &App, dial: Spectrum, value: Option<f32>, t: tok::Theme) -> El {
    const TRACK: f32 = 216.0;
    let (left, right) = dial.ends();
    let pos = value.unwrap_or(0.5) * TRACK;
    let track = el()
        .key(format!("dial-{}", dial.slug()))
        .role("slider")
        .label(format!("{left} to {right}"))
        .w_full()
        .h(22.0)
        .on_drag(move |s: &mut App, d: &mut Drag, _| {
            if d.phase == Phase::Down {
                d.data = value.unwrap_or(0.5);
            }
            let v = slider_value(d, d.data, TRACK);
            s.set_chip(dial, Some(v));
        })
        .child(
            el().key("track").absolute(0.0, 9.0).w(TRACK).h(3.0).radius(2.0).fill(if value.is_some() { t.knob } else { t.control_track }),
        );
    let track = if value.is_some() {
        track.child(
            el()
                .key("knob")
                .absolute((pos - 6.0).clamp(0.0, TRACK - 12.0), 3.5)
                .size(12.0, 14.0)
                .radius(6.0)
                .fill(t.knob),
        )
    } else {
        track
    };

    let mut header = row()
        .w_full()
        .justify_between()
        .items_center()
        .child(text(left).font(type_::MONO_CAPTION).color(t.text_secondary))
        .child(text(right).font(type_::MONO_CAPTION).color(t.text_secondary));
    if value.is_some() {
        header = header.child(
            text("clear")
                .key("clear")
                .role("button")
                .font(TextStyle::new(10.0, 500.0))
                .color(t.text_secondary)
                .px(4.0)
                .radius(radius::SM)
                .hover(t.hover)
                .on_press(move |s: &mut App| s.set_chip(dial, None)),
        );
    }

    col().key(format!("dialer-{}", dial.slug())).w_full().gap(1.0).child(header).child(track)
}
