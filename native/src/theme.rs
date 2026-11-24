//! Linear / Raycast-inspired design tokens and reusable micro-primitives for the native Rune UI.
//!
//! Four deliberate surface tiers separated by 1px hairlines, crisp keyboard keycaps (`kbd`),
//! indigo focus/primary accent, and restrained semantic signals for Keep / Flinch / Contrast.

use rune::{el, text, Color, El, TextStyle};

/// Convert a 24-bit RGB hex literal into a Rune `Color`.
pub const fn rgb(hex: u32) -> Color {
    Color {
        r: ((hex >> 16) & 0xFF) as f32 / 255.0,
        g: ((hex >> 8) & 0xFF) as f32 / 255.0,
        b: (hex & 0xFF) as f32 / 255.0,
        a: 1.0,
    }
}

pub const fn rgba(hex: u32, a: f32) -> Color {
    let mut c = rgb(hex);
    c.a = a;
    c
}

#[derive(Clone, Copy, Debug)]
pub struct Palette {
    /// Deep recessed canvas / stage background.
    pub bg_canvas: Color,
    /// Side rails (shelf & inspector) and chrome bars.
    pub bg_rail: Color,
    /// Elevated interactive cards, inputs, segmented control wells.
    pub bg_surface: Color,
    /// Active item / selected segment / subtle hover surface.
    pub bg_elevated: Color,
    /// Hover tint for interactive rows & buttons.
    pub hover: Color,
    /// 1px structural hairline divider & subtle border.
    pub border_hair: Color,
    /// Emphasized border for focused controls & active counterfactual frame.
    pub border_strong: Color,
    /// Primary readable ink.
    pub ink_primary: Color,
    /// Secondary descriptive ink.
    pub ink_secondary: Color,
    /// Quiet muted ink for hints & empty copy.
    pub ink_muted: Color,
    /// Linear indigo accent for primary CTA and active focus.
    pub accent: Color,
    pub accent_soft: Color,
    pub on_accent: Color,
    /// Semantic signals: Keep / Flinch / Contrast.
    pub keep: Color,
    pub keep_soft: Color,
    pub flinch: Color,
    pub flinch_soft: Color,
    pub contrast: Color,
    /// Raycast-style keyboard keycap surface & border.
    pub kbd_bg: Color,
    pub kbd_border: Color,
}

pub fn palette(dark: bool) -> Palette {
    if dark {
        Palette {
            bg_canvas: rgb(0x0A0B0E),
            bg_rail: rgb(0x0F1116),
            bg_surface: rgb(0x151820),
            bg_elevated: rgb(0x1D212B),
            hover: rgb(0x1A1E27),
            border_hair: rgb(0x1F232D),
            border_strong: rgb(0x313746),
            ink_primary: rgb(0xECEEF2),
            ink_secondary: rgb(0x949AA8),
            ink_muted: rgb(0x636978),
            accent: rgb(0x5E6AD2),
            accent_soft: rgba(0x5E6AD2, 0.16),
            on_accent: rgb(0xF7F8FA),
            keep: rgb(0x34C77B),
            keep_soft: rgba(0x34C77B, 0.14),
            flinch: rgb(0xE5534B),
            flinch_soft: rgba(0xE5534B, 0.14),
            contrast: rgb(0xE5A93C),
            kbd_bg: rgb(0x191C24),
            kbd_border: rgb(0x2B303E),
        }
    } else {
        Palette {
            bg_canvas: rgb(0xF3F4F6),
            bg_rail: rgb(0xFAFAFC),
            bg_surface: rgb(0xFFFFFF),
            bg_elevated: rgb(0xEAECEF),
            hover: rgb(0xEEF0F4),
            border_hair: rgb(0xE2E5EC),
            border_strong: rgb(0xC5CBD8),
            ink_primary: rgb(0x111318),
            ink_secondary: rgb(0x596070),
            ink_muted: rgb(0x8890A0),
            accent: rgb(0x5E6AD2),
            accent_soft: rgba(0x5E6AD2, 0.12),
            on_accent: rgb(0xFFFFFF),
            keep: rgb(0x1F9D5D),
            keep_soft: rgba(0x1F9D5D, 0.12),
            flinch: rgb(0xD63E36),
            flinch_soft: rgba(0xD63E36, 0.12),
            contrast: rgb(0xC7871A),
            kbd_bg: rgb(0xF0F2F6),
            kbd_border: rgb(0xD5DAE4),
        }
    }
}

/// Raycast-style keyboard keycap badge (`1`, `⌘E`, `V`, etc.).
pub fn kbd(label: &str, p: Palette) -> El {
    text(label)
        .font(TextStyle::new(10.0, 500.0))
        .color(p.ink_secondary)
        .px(5.0)
        .py(1.5)
        .radius(4.0)
        .fill(p.kbd_bg)
        .border(1.0, p.kbd_border)
}

/// Crisp circular status dot.
pub fn dot(size: f32, color: Color) -> El {
    el().size(size, size).radius(size * 0.5).fill(color)
}

/// Quiet sentence-case section label.
pub fn section_label(label: &str, p: Palette) -> El {
    text(label)
        .font(TextStyle::new(11.5, 500.0))
        .color(p.ink_secondary)
}

/// 1px horizontal hairline divider.
pub fn divider(p: Palette) -> El {
    el().w_full().h(1.0).fill(p.border_hair)
}
