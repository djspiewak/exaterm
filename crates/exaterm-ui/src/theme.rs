use crate::supervision::BattleCardStatus;

#[derive(Clone, Debug, PartialEq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Gradient {
    pub top: Color,
    pub bottom: Color,
}

#[derive(Clone, Debug)]
pub struct Shadow {
    pub offset_y: f32,
    pub blur: f32,
    pub color: Color,
}

#[derive(Clone, Debug)]
pub struct CardTheme {
    pub border_radius: f32,
    pub border_color: Color,
    pub background: Gradient,
    pub shadow: Shadow,
    pub min_width: f32,
    pub min_height: f32,
}

#[derive(Clone, Debug)]
pub struct StatusChipTheme {
    pub text_color: Color,
    pub background: Color,
    pub border_color: Color,
}

#[derive(Clone, Debug)]
pub struct FontSpec {
    pub size: f32,
    pub weight: u16,
    pub letter_spacing: f32,
    pub line_height: Option<f32>,
    pub monospace: bool,
}

const CARD_BORDER_RADIUS: f32 = 24.0;
const CARD_MIN_WIDTH: f32 = 392.0;
const CARD_MIN_HEIGHT: f32 = 220.0;

const CARD_SHADOW: Shadow = Shadow {
    offset_y: 24.0,
    blur: 46.0,
    color: Color {
        r: 0,
        g: 0,
        b: 0,
        a: 0.28,
    },
};

fn make_card(top: Color, bottom: Color, border: Color) -> CardTheme {
    CardTheme {
        border_radius: CARD_BORDER_RADIUS,
        border_color: border,
        background: Gradient { top, bottom },
        shadow: CARD_SHADOW,
        min_width: CARD_MIN_WIDTH,
        min_height: CARD_MIN_HEIGHT,
    }
}

pub fn card_theme(status: BattleCardStatus) -> CardTheme {
    match status {
        BattleCardStatus::Idle => make_card(
            Color {
                r: 21,
                g: 24,
                b: 30,
                a: 0.98,
            },
            Color {
                r: 12,
                g: 14,
                b: 19,
                a: 0.97,
            },
            Color {
                r: 21,
                g: 24,
                b: 30,
                a: 0.96,
            },
        ),
        BattleCardStatus::Stopped => make_card(
            Color {
                r: 54,
                g: 43,
                b: 11,
                a: 0.98,
            },
            Color {
                r: 23,
                g: 21,
                b: 9,
                a: 0.97,
            },
            Color {
                r: 54,
                g: 43,
                b: 11,
                a: 0.96,
            },
        ),
        BattleCardStatus::Active => make_card(
            Color {
                r: 14,
                g: 33,
                b: 52,
                a: 0.98,
            },
            Color {
                r: 9,
                g: 18,
                b: 31,
                a: 0.97,
            },
            Color {
                r: 14,
                g: 33,
                b: 52,
                a: 0.96,
            },
        ),
        BattleCardStatus::Thinking | BattleCardStatus::Working => make_card(
            Color {
                r: 9,
                g: 44,
                b: 29,
                a: 0.98,
            },
            Color {
                r: 9,
                g: 23,
                b: 16,
                a: 0.97,
            },
            Color {
                r: 9,
                g: 44,
                b: 29,
                a: 0.96,
            },
        ),
        BattleCardStatus::Blocked | BattleCardStatus::Failed => make_card(
            Color {
                r: 55,
                g: 18,
                b: 22,
                a: 0.98,
            },
            Color {
                r: 27,
                g: 11,
                b: 14,
                a: 0.97,
            },
            Color {
                r: 55,
                g: 18,
                b: 22,
                a: 0.96,
            },
        ),
        BattleCardStatus::Complete => make_card(
            Color {
                r: 11,
                g: 40,
                b: 41,
                a: 0.98,
            },
            Color {
                r: 7,
                g: 20,
                b: 22,
                a: 0.97,
            },
            Color {
                r: 11,
                g: 40,
                b: 41,
                a: 0.96,
            },
        ),
        BattleCardStatus::Detached => make_card(
            Color {
                r: 36,
                g: 18,
                b: 51,
                a: 0.98,
            },
            Color {
                r: 16,
                g: 9,
                b: 25,
                a: 0.97,
            },
            Color {
                r: 36,
                g: 18,
                b: 51,
                a: 0.96,
            },
        ),
    }
}

pub fn status_chip_theme(status: BattleCardStatus) -> StatusChipTheme {
    match status {
        BattleCardStatus::Idle => StatusChipTheme {
            text_color: Color {
                r: 203,
                g: 213,
                b: 225,
                a: 1.0,
            },
            background: Color {
                r: 71,
                g: 85,
                b: 105,
                a: 0.18,
            },
            border_color: Color {
                r: 148,
                g: 163,
                b: 184,
                a: 0.22,
            },
        },
        BattleCardStatus::Stopped => StatusChipTheme {
            text_color: Color {
                r: 253,
                g: 230,
                b: 138,
                a: 1.0,
            },
            background: Color {
                r: 120,
                g: 87,
                b: 10,
                a: 0.22,
            },
            border_color: Color {
                r: 250,
                g: 204,
                b: 21,
                a: 0.28,
            },
        },
        BattleCardStatus::Active => StatusChipTheme {
            text_color: Color {
                r: 147,
                g: 197,
                b: 253,
                a: 1.0,
            },
            background: Color {
                r: 33,
                g: 82,
                b: 145,
                a: 0.22,
            },
            border_color: Color {
                r: 96,
                g: 165,
                b: 250,
                a: 0.26,
            },
        },
        BattleCardStatus::Thinking | BattleCardStatus::Working => StatusChipTheme {
            text_color: Color {
                r: 134,
                g: 239,
                b: 172,
                a: 1.0,
            },
            background: Color {
                r: 17,
                g: 88,
                b: 51,
                a: 0.24,
            },
            border_color: Color {
                r: 74,
                g: 222,
                b: 128,
                a: 0.24,
            },
        },
        BattleCardStatus::Blocked | BattleCardStatus::Failed => StatusChipTheme {
            text_color: Color {
                r: 252,
                g: 165,
                b: 165,
                a: 1.0,
            },
            background: Color {
                r: 114,
                g: 28,
                b: 35,
                a: 0.24,
            },
            border_color: Color {
                r: 248,
                g: 113,
                b: 113,
                a: 0.24,
            },
        },
        BattleCardStatus::Complete => StatusChipTheme {
            text_color: Color {
                r: 153,
                g: 246,
                b: 228,
                a: 1.0,
            },
            background: Color {
                r: 16,
                g: 77,
                b: 77,
                a: 0.22,
            },
            border_color: Color {
                r: 94,
                g: 234,
                b: 212,
                a: 0.24,
            },
        },
        BattleCardStatus::Detached => StatusChipTheme {
            text_color: Color {
                r: 233,
                g: 213,
                b: 255,
                a: 1.0,
            },
            background: Color {
                r: 74,
                g: 34,
                b: 112,
                a: 0.22,
            },
            border_color: Color {
                r: 192,
                g: 132,
                b: 252,
                a: 0.24,
            },
        },
    }
}

pub fn card_title_font() -> FontSpec {
    FontSpec {
        size: 18.0,
        weight: 800,
        letter_spacing: 0.0,
        line_height: None,
        monospace: false,
    }
}

pub fn card_subtitle_font() -> FontSpec {
    FontSpec {
        size: 12.0,
        weight: 400,
        letter_spacing: 0.04,
        line_height: None,
        monospace: false,
    }
}

pub fn card_status_font() -> FontSpec {
    FontSpec {
        size: 10.0,
        weight: 800,
        letter_spacing: 0.08,
        line_height: None,
        monospace: false,
    }
}

pub fn card_recency_font() -> FontSpec {
    FontSpec {
        size: 12.0,
        weight: 700,
        letter_spacing: 0.03,
        line_height: None,
        monospace: false,
    }
}

pub fn card_headline_font() -> FontSpec {
    FontSpec {
        size: 20.0,
        weight: 800,
        letter_spacing: 0.0,
        line_height: Some(1.12),
        monospace: false,
    }
}

pub fn card_detail_font() -> FontSpec {
    FontSpec {
        size: 15.0,
        weight: 650,
        letter_spacing: 0.0,
        line_height: Some(1.25),
        monospace: false,
    }
}

pub fn card_evidence_font() -> FontSpec {
    FontSpec {
        size: 12.0,
        weight: 400,
        letter_spacing: 0.0,
        line_height: None,
        monospace: true,
    }
}

pub fn card_alert_font() -> FontSpec {
    FontSpec {
        size: 11.0,
        weight: 600,
        letter_spacing: 0.0,
        line_height: Some(1.2),
        monospace: false,
    }
}

pub fn bar_caption_font() -> FontSpec {
    FontSpec {
        size: 10.0,
        weight: 400,
        letter_spacing: 0.08,
        line_height: None,
        monospace: false,
    }
}

pub fn bar_reason_font() -> FontSpec {
    FontSpec {
        size: 10.0,
        weight: 400,
        letter_spacing: 0.0,
        line_height: Some(1.2),
        monospace: false,
    }
}

pub fn focus_title_font() -> FontSpec {
    FontSpec {
        size: 20.0,
        weight: 800,
        letter_spacing: 0.0,
        line_height: None,
        monospace: false,
    }
}

pub fn focus_subtitle_font() -> FontSpec {
    FontSpec {
        size: 14.0,
        weight: 400,
        letter_spacing: 0.0,
        line_height: None,
        monospace: false,
    }
}

pub fn scrollback_line_font() -> FontSpec {
    FontSpec {
        size: 11.0,
        weight: 400,
        letter_spacing: 0.0,
        line_height: Some(1.1),
        monospace: true,
    }
}

pub fn terminal_font() -> FontSpec {
    FontSpec {
        size: 13.0,
        weight: 400,
        letter_spacing: 0.0,
        line_height: None,
        monospace: true,
    }
}

// ---------------------------------------------------------------------------
// Shared color accessors — single source of truth for CSS and macOS renderer
// ---------------------------------------------------------------------------

// Text colors
pub fn title_color() -> Color {
    Color { r: 248, g: 250, b: 252, a: 1.0 }
}

pub fn headline_color() -> Color {
    Color { r: 248, g: 250, b: 252, a: 1.0 }
}

pub fn detail_color() -> Color {
    Color { r: 226, g: 234, b: 242, a: 0.94 }
}

pub fn subtitle_color() -> Color {
    Color { r: 196, g: 208, b: 222, a: 0.66 }
}

pub fn recency_color() -> Color {
    Color { r: 188, g: 201, b: 216, a: 0.88 }
}

pub fn alert_color() -> Color {
    Color { r: 202, g: 214, b: 227, a: 0.78 }
}

pub fn scrollback_line_color() -> Color {
    Color { r: 202, g: 214, b: 227, a: 0.88 }
}

pub fn evidence_text_color() -> Color {
    Color { r: 198, g: 212, b: 227, a: 0.88 }
}

pub fn bar_caption_color() -> Color {
    Color { r: 186, g: 200, b: 214, a: 0.62 }
}

pub fn bar_reason_color() -> Color {
    Color { r: 186, g: 200, b: 214, a: 0.56 }
}

// Structural colors
pub fn bar_empty_color() -> Color {
    Color { r: 163, g: 175, b: 194, a: 0.14 }
}

pub fn selected_card_border() -> Color {
    Color { r: 113, g: 197, b: 255, a: 0.98 }
}

pub fn transcript_bg() -> Color {
    Color { r: 8, g: 14, b: 22, a: 0.34 }
}

pub fn transcript_border() -> Color {
    Color { r: 173, g: 188, b: 204, a: 0.08 }
}

// Bar gradients
pub fn bar_calm_gradient() -> Gradient {
    Gradient {
        top: Color { r: 110, g: 231, b: 183, a: 0.88 },
        bottom: Color { r: 52, g: 211, b: 153, a: 0.92 },
    }
}

pub fn bar_watch_gradient() -> Gradient {
    Gradient {
        top: Color { r: 250, g: 204, b: 21, a: 0.88 },
        bottom: Color { r: 251, g: 146, b: 60, a: 0.92 },
    }
}

pub fn bar_alert_gradient() -> Gradient {
    Gradient {
        top: Color { r: 248, g: 113, b: 113, a: 0.9 },
        bottom: Color { r: 239, g: 68, b: 68, a: 0.94 },
    }
}

// Attention chip backgrounds (by fill level 1-5)
pub fn attention_chip_bg(fill: usize) -> Color {
    match fill.clamp(1, 5) {
        1 => Color { r: 17, g: 88, b: 51, a: 0.24 },
        2 => Color { r: 33, g: 82, b: 145, a: 0.22 },
        3 => Color { r: 120, g: 87, b: 10, a: 0.22 },
        4 => Color { r: 114, g: 28, b: 35, a: 0.24 },
        _ => Color { r: 130, g: 35, b: 35, a: 0.32 },
    }
}

// Nudge state colors (text, background)
pub fn nudge_off_colors() -> (Color, Color) {
    (
        Color { r: 214, g: 222, b: 230, a: 0.84 },
        Color { r: 84, g: 97, b: 112, a: 0.18 },
    )
}

pub fn nudge_armed_colors() -> (Color, Color) {
    (
        Color { r: 253, g: 230, b: 138, a: 1.0 },
        Color { r: 120, g: 87, b: 10, a: 0.22 },
    )
}

pub fn nudge_cooldown_colors() -> (Color, Color) {
    (
        Color { r: 147, g: 197, b: 253, a: 1.0 },
        Color { r: 33, g: 82, b: 145, a: 0.22 },
    )
}

// Focus view
pub fn focus_background() -> Color {
    Color { r: 5, g: 10, b: 18, a: 1.0 }
}

pub fn focus_card_border() -> Color {
    Color { r: 18, g: 46, b: 64, a: 1.0 }
}

pub fn focus_terminal_slot_bg() -> Color {
    Color { r: 4, g: 8, b: 12, a: 0.94 }
}

// Control chips (focus rail cards)
pub fn control_chip_state_font() -> FontSpec {
    FontSpec {
        size: 10.0,
        weight: 800,
        letter_spacing: 0.08,
        line_height: None,
        monospace: false,
    }
}

/// Returns (text, background, border) for the Off control state.
pub fn control_off_colors() -> (Color, Color, Color) {
    (
        Color { r: 214, g: 222, b: 230, a: 0.84 },
        Color { r: 84, g: 97, b: 112, a: 0.18 },
        Color { r: 163, g: 175, b: 194, a: 0.16 },
    )
}

/// Returns (text, background, border) for the Armed control state.
pub fn control_armed_colors() -> (Color, Color, Color) {
    (
        Color { r: 253, g: 230, b: 138, a: 1.0 },
        Color { r: 120, g: 87, b: 10, a: 0.22 },
        Color { r: 250, g: 204, b: 21, a: 0.22 },
    )
}

/// Returns (text, background, border) for the Cooldown control state.
pub fn control_cooldown_colors() -> (Color, Color, Color) {
    (
        Color { r: 147, g: 197, b: 253, a: 1.0 },
        Color { r: 33, g: 82, b: 145, a: 0.22 },
        Color { r: 96, g: 165, b: 250, a: 0.2 },
    )
}

// Status bar
pub fn status_bar_bg() -> Color {
    Color { r: 5, g: 10, b: 18, a: 0.96 }
}

pub fn status_bar_text_color() -> Color {
    Color { r: 196, g: 208, b: 222, a: 0.78 }
}

pub fn terminal_foreground_color() -> Color {
    Color {
        r: 204,
        g: 204,
        b: 204,
        a: 1.0,
    }
}

pub fn terminal_background_color() -> Color {
    Color {
        r: 0,
        g: 0,
        b: 0,
        a: 1.0,
    }
}

pub fn terminal_cursor_color() -> Color {
    Color {
        r: 134,
        g: 239,
        b: 172,
        a: 1.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::supervision::BattleCardStatus;

    const ALL_STATUSES: &[BattleCardStatus] = &[
        BattleCardStatus::Idle,
        BattleCardStatus::Stopped,
        BattleCardStatus::Active,
        BattleCardStatus::Thinking,
        BattleCardStatus::Working,
        BattleCardStatus::Blocked,
        BattleCardStatus::Failed,
        BattleCardStatus::Complete,
        BattleCardStatus::Detached,
    ];

    #[test]
    fn every_status_has_card_theme() {
        for &status in ALL_STATUSES {
            let _ = card_theme(status);
        }
    }

    #[test]
    fn every_status_has_chip_theme() {
        for &status in ALL_STATUSES {
            let _ = status_chip_theme(status);
        }
    }

    #[test]
    fn card_title_font_matches_css() {
        let font = card_title_font();
        assert_eq!(font.weight, 800);
        assert!((font.size - 18.0).abs() < 0.01);
        assert!(!font.monospace);
    }

    #[test]
    fn card_active_gradient_matches_css() {
        // CSS: .card-active background: linear-gradient(180deg, rgba(14, 33, 52, 0.98) 0%, rgba(9, 18, 31, 0.97) 100%)
        let theme = card_theme(BattleCardStatus::Active);
        assert_eq!(
            theme.background.top,
            Color {
                r: 14,
                g: 33,
                b: 52,
                a: 0.98
            }
        );
        assert_eq!(
            theme.background.bottom,
            Color {
                r: 9,
                g: 18,
                b: 31,
                a: 0.97
            }
        );
    }

    #[test]
    fn battle_active_chip_matches_css() {
        // CSS: .battle-active { color: #93c5fd; background: rgba(33, 82, 145, 0.22); border-color: rgba(96, 165, 250, 0.26); }
        let chip = status_chip_theme(BattleCardStatus::Active);
        assert_eq!(
            chip.text_color,
            Color {
                r: 147,
                g: 197,
                b: 253,
                a: 1.0
            }
        );
        assert_eq!(
            chip.background,
            Color {
                r: 33,
                g: 82,
                b: 145,
                a: 0.22
            }
        );
        assert_eq!(
            chip.border_color,
            Color {
                r: 96,
                g: 165,
                b: 250,
                a: 0.26
            }
        );
    }

    #[test]
    fn evidence_font_is_monospace() {
        assert!(card_evidence_font().monospace);
    }

    #[test]
    fn scrollback_line_font_is_monospace() {
        assert!(scrollback_line_font().monospace);
    }

    #[test]
    fn terminal_font_is_monospace() {
        let font = terminal_font();
        assert!(font.monospace);
        assert!((font.size - 13.0).abs() < 0.01);
    }

    #[test]
    fn terminal_palette_is_opaque() {
        assert_eq!(terminal_foreground_color().a, 1.0);
        assert_eq!(terminal_background_color().a, 1.0);
        assert_eq!(terminal_cursor_color().a, 1.0);
    }
}
