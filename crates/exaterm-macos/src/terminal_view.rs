// Cached render state for card UI rendering (fonts, colors).

use std::collections::BTreeMap;

use crate::style::{self, NormalizedColor};

use objc2::rc::Retained;
use objc2_app_kit::{NSColor, NSFont};

use exaterm_ui::presentation::NudgeStateTone;
use exaterm_ui::supervision::BattleCardStatus;
use exaterm_ui::theme::{self as theme};

fn ns_color(c: &NormalizedColor) -> Retained<NSColor> {
    NSColor::colorWithSRGBRed_green_blue_alpha(c.r, c.g, c.b, c.a)
}

/// All statuses we iterate over to build per-status caches.
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

/// Convenience: create cached NSColor/NSFont objects from the theme.
///
/// All theme-derived fonts and colors are computed once and cached for use in
/// rendering functions. No hardcoded colors exist outside this constructor.
pub struct TerminalRenderState {
    // Card UI fonts (from theme).
    pub title_font: Retained<NSFont>,
    pub status_font: Retained<NSFont>,
    pub recency_font: Retained<NSFont>,
    pub headline_font: Retained<NSFont>,
    pub detail_font: Retained<NSFont>,
    pub alert_font: Retained<NSFont>,
    pub scrollback_font: Retained<NSFont>,
    pub bar_caption_font: Retained<NSFont>,
    pub bar_reason_font: Retained<NSFont>,

    // Card UI colors (from theme CSS values).
    pub title_color: Retained<NSColor>,
    pub headline_color: Retained<NSColor>,
    pub detail_color: Retained<NSColor>,
    pub alert_color: Retained<NSColor>,
    pub recency_color: Retained<NSColor>,
    pub scrollback_color: Retained<NSColor>,
    pub selected_bg: Retained<NSColor>,
    pub attention_chip_text: Retained<NSColor>,
    pub transcript_bg: Retained<NSColor>,
    pub transcript_border: Retained<NSColor>,
    pub bar_caption_color: Retained<NSColor>,
    pub bar_reason_color: Retained<NSColor>,
    pub bar_empty: Retained<NSColor>,

    // Per-status cached colors: discriminant -> (chip_text_color, chip_bg_color).
    pub status_chip_colors: BTreeMap<u8, (Retained<NSColor>, Retained<NSColor>)>,
    // Per-status card background: discriminant -> (top, bottom) gradient colors.
    pub card_bg_colors: BTreeMap<u8, (Retained<NSColor>, Retained<NSColor>)>,
    pub attention_bg_colors: BTreeMap<usize, Retained<NSColor>>,
    pub nudge_colors: BTreeMap<u8, (Retained<NSColor>, Retained<NSColor>)>,

    // Control chip colors: nudge discriminant -> (text, bg, border).
    pub control_chip_colors: BTreeMap<u8, (Retained<NSColor>, Retained<NSColor>, Retained<NSColor>)>,

    // Attention bar gradient endpoints: (left, right) for calm, watch, alert.
    pub bar_calm_left: Retained<NSColor>,
    pub bar_calm_right: Retained<NSColor>,
    pub bar_watch_left: Retained<NSColor>,
    pub bar_watch_right: Retained<NSColor>,
    pub bar_alert_left: Retained<NSColor>,
    pub bar_alert_right: Retained<NSColor>,
}

/// Return a `u8` discriminant for a `BattleCardStatus` variant (used as map key).
fn status_discriminant(s: BattleCardStatus) -> u8 {
    match s {
        BattleCardStatus::Idle => 0,
        BattleCardStatus::Stopped => 1,
        BattleCardStatus::Active => 2,
        BattleCardStatus::Thinking => 3,
        BattleCardStatus::Working => 4,
        BattleCardStatus::Blocked => 5,
        BattleCardStatus::Failed => 6,
        BattleCardStatus::Complete => 7,
        BattleCardStatus::Detached => 8,
    }
}

impl TerminalRenderState {
    pub fn new() -> Self {
        // Card UI fonts from theme specs.
        let title_font = style::font_from_spec(&theme::card_title_font());
        let status_font = style::font_from_spec(&theme::card_status_font());
        let recency_font = style::font_from_spec(&theme::card_recency_font());
        let headline_font = style::font_from_spec(&theme::card_headline_font());
        let detail_font = style::font_from_spec(&theme::card_detail_font());
        let alert_font = style::font_from_spec(&theme::card_alert_font());
        let scrollback_font = style::font_from_spec(&theme::scrollback_line_font());
        let bar_caption_font = style::font_from_spec(&theme::bar_caption_font());
        let bar_reason_font = style::font_from_spec(&theme::bar_reason_font());

        // Card UI colors from theme CSS values.
        let title_color = style::color_to_nscolor(&theme::title_color());
        let headline_color = style::color_to_nscolor(&theme::headline_color());
        let detail_color = style::color_to_nscolor(&theme::detail_color());
        let alert_color = style::color_to_nscolor(&theme::alert_color());
        let recency_color = style::color_to_nscolor(&theme::recency_color());
        let scrollback_color = style::color_to_nscolor(&theme::scrollback_line_color());
        let selected_bg = style::color_to_nscolor(&theme::selected_card_border());
        let attention_chip_text = style::color_to_nscolor(&theme::title_color());
        let transcript_bg = style::color_to_nscolor(&theme::transcript_bg());
        let transcript_border = style::color_to_nscolor(&theme::transcript_border());
        let bar_caption_color = style::color_to_nscolor(&theme::bar_caption_color());
        let bar_reason_color = style::color_to_nscolor(&theme::bar_reason_color());
        let bar_empty = style::color_to_nscolor(&theme::bar_empty_color());

        // Per-status cached colors.
        let mut status_chip_colors = BTreeMap::new();
        let mut card_bg_colors = BTreeMap::new();
        let mut attention_bg_colors = BTreeMap::new();
        let mut nudge_colors = BTreeMap::new();
        for &status in ALL_STATUSES {
            let disc = status_discriminant(status);
            let chip = theme::status_chip_theme(status);
            status_chip_colors.insert(
                disc,
                (
                    style::color_to_nscolor(&chip.text_color),
                    style::color_to_nscolor(&chip.background),
                ),
            );
            let layer = style::card_layer_style(status);
            card_bg_colors.insert(
                disc,
                (ns_color(&layer.background_top), ns_color(&layer.background_bottom)),
            );
        }
        attention_bg_colors.insert(1, style::color_to_nscolor(&theme::attention_chip_bg(1)));
        attention_bg_colors.insert(2, style::color_to_nscolor(&theme::attention_chip_bg(2)));
        attention_bg_colors.insert(3, style::color_to_nscolor(&theme::attention_chip_bg(3)));
        attention_bg_colors.insert(4, style::color_to_nscolor(&theme::attention_chip_bg(4)));
        attention_bg_colors.insert(5, style::color_to_nscolor(&theme::attention_chip_bg(5)));
        {
            let (fg, bg) = theme::nudge_off_colors();
            nudge_colors.insert(0, (style::color_to_nscolor(&fg), style::color_to_nscolor(&bg)));
        }
        {
            let (fg, bg) = theme::nudge_armed_colors();
            nudge_colors.insert(1, (style::color_to_nscolor(&fg), style::color_to_nscolor(&bg)));
        }
        {
            let (fg, bg) = theme::nudge_cooldown_colors();
            nudge_colors.insert(2, (style::color_to_nscolor(&fg), style::color_to_nscolor(&bg)));
        }

        let mut control_chip_colors = BTreeMap::new();
        {
            let (t, b, bd) = theme::control_off_colors();
            control_chip_colors.insert(0, (style::color_to_nscolor(&t), style::color_to_nscolor(&b), style::color_to_nscolor(&bd)));
        }
        {
            let (t, b, bd) = theme::control_armed_colors();
            control_chip_colors.insert(1, (style::color_to_nscolor(&t), style::color_to_nscolor(&b), style::color_to_nscolor(&bd)));
        }
        {
            let (t, b, bd) = theme::control_cooldown_colors();
            control_chip_colors.insert(2, (style::color_to_nscolor(&t), style::color_to_nscolor(&b), style::color_to_nscolor(&bd)));
        }
        let calm = theme::bar_calm_gradient();
        let watch = theme::bar_watch_gradient();
        let alert = theme::bar_alert_gradient();

        Self {
            title_font,
            status_font,
            recency_font,
            headline_font,
            detail_font,
            alert_font,
            scrollback_font,
            bar_caption_font,
            bar_reason_font,
            title_color,
            headline_color,
            detail_color,
            alert_color,
            recency_color,
            scrollback_color,
            selected_bg,
            attention_chip_text,
            transcript_bg,
            transcript_border,
            bar_caption_color,
            bar_reason_color,
            bar_empty,
            status_chip_colors,
            card_bg_colors,
            attention_bg_colors,
            nudge_colors,
            control_chip_colors,
            bar_calm_left: style::color_to_nscolor(&calm.top),
            bar_calm_right: style::color_to_nscolor(&calm.bottom),
            bar_watch_left: style::color_to_nscolor(&watch.top),
            bar_watch_right: style::color_to_nscolor(&watch.bottom),
            bar_alert_left: style::color_to_nscolor(&alert.top),
            bar_alert_right: style::color_to_nscolor(&alert.bottom),
        }
    }

    /// Look up the cached chip text color for a given status.
    pub fn chip_text_color(&self, status: BattleCardStatus) -> &Retained<NSColor> {
        &self.status_chip_colors[&status_discriminant(status)].0
    }

    /// Look up the cached chip background color for a given status.
    pub fn chip_bg_color(&self, status: BattleCardStatus) -> &Retained<NSColor> {
        &self.status_chip_colors[&status_discriminant(status)].1
    }

    /// Look up the cached card background top color for a given status.
    pub fn card_bg_top(&self, status: BattleCardStatus) -> &Retained<NSColor> {
        &self.card_bg_colors[&status_discriminant(status)].0
    }

    /// Look up the cached card background bottom color for a given status.
    pub fn card_bg_bottom(&self, status: BattleCardStatus) -> &Retained<NSColor> {
        &self.card_bg_colors[&status_discriminant(status)].1
    }

    pub fn attention_chip_bg(&self, fill: usize) -> &Retained<NSColor> {
        &self.attention_bg_colors[&fill.clamp(1, 5)]
    }

    pub fn nudge_text_color(&self, tone: NudgeStateTone) -> &Retained<NSColor> {
        &self.nudge_colors[&nudge_discriminant(tone)].0
    }

    pub fn nudge_bg_color(&self, tone: NudgeStateTone) -> &Retained<NSColor> {
        &self.nudge_colors[&nudge_discriminant(tone)].1
    }

    /// Look up the cached control chip colors (text, bg, border) for a nudge tone.
    pub fn control_chip(&self, tone: NudgeStateTone) -> (&Retained<NSColor>, &Retained<NSColor>, &Retained<NSColor>) {
        let entry = &self.control_chip_colors[&nudge_discriminant(tone)];
        (&entry.0, &entry.1, &entry.2)
    }

    /// Return the (left, right) gradient colors for an attention bar segment.
    /// Fill 1-2 → calm, 3 → watch, 4-5 → alert.
    pub fn attention_bar_gradient(&self, fill: usize) -> (&Retained<NSColor>, &Retained<NSColor>) {
        match fill.clamp(1, 5) {
            1 | 2 => (&self.bar_calm_left, &self.bar_calm_right),
            3 => (&self.bar_watch_left, &self.bar_watch_right),
            _ => (&self.bar_alert_left, &self.bar_alert_right),
        }
    }
}

fn nudge_discriminant(tone: NudgeStateTone) -> u8 {
    match tone {
        NudgeStateTone::Off => 0,
        NudgeStateTone::Armed => 1,
        NudgeStateTone::Cooldown => 2,
    }
}
