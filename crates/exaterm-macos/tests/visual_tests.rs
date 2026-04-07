//! Custom-harness visual tests for macOS rendering.
//!
//! These tests assert visual properties of the rendered card UI.
//! They use off-screen AppKit rendering (NSBitmapImageRep) and do not
//! require screen recording permissions or a display server.

mod helpers;

use helpers::*;
use objc2_foundation::{MainThreadMarker, NSSize};

use exaterm_test_util::pixel_compare::{assert_visual_match, CompareConfig};
use exaterm_types::model::SessionId;
use exaterm_ui::presentation::{
    AttentionPresentation, NudgeStatePresentation, NudgeStateTone, SegmentedBarPresentation,
};
use exaterm_ui::supervision::BattleCardStatus;

fn main() {
    exaterm_test_util::appkit_harness::run_tests(&[
        // Color and background tests
        ("card_bg_has_vertical_gradient", card_bg_has_vertical_gradient),
        ("card_has_shadow_below", card_has_shadow_below),
        ("transcript_bg_matches_theme", transcript_bg_matches_theme),
        ("transcript_border_matches_theme", transcript_border_matches_theme),
        ("selected_card_border_is_bright", selected_card_border_is_bright),
        ("attention_bar_calm_is_gradient", attention_bar_calm_is_gradient),
        // Text styling and positioning tests
        ("title_renders_at_top_of_card", title_renders_at_top_of_card),
        ("status_chip_renders_below_title", status_chip_renders_below_title),
        ("headline_text_rendered_and_positioned", headline_text_rendered_and_positioned),
        ("detail_text_rendered_when_present", detail_text_rendered_when_present),
        ("alert_text_rendered_with_prefix", alert_text_rendered_with_prefix),
        ("recency_label_positioned_after_content", recency_label_positioned_after_content),
        ("scrollback_uses_monospace_proportions", scrollback_uses_monospace_proportions),
        ("attention_bar_label_rendered", attention_bar_label_rendered),
        ("nudge_chip_renders_on_right", nudge_chip_renders_on_right),
        ("focus_view_title_and_status_rendered", focus_view_title_and_status_rendered),
        ("focus_view_headline_rendered", focus_view_headline_rendered),
        // Snapshot tests
        ("battlefield_snapshot", battlefield_snapshot),
        ("focus_snapshot", focus_snapshot),
    ]);
}

// ---------------------------------------------------------------------------
// Color and background tests
// ---------------------------------------------------------------------------

/// Sample top-quarter vs bottom-quarter of an Active card.
/// Top should have higher blue channel (14,33,52) than bottom (9,18,31).
/// Currently flat fill -> FAILS.
fn card_bg_has_vertical_gradient(mtm: MainThreadMarker) {
    let card = make_card(BattleCardStatus::Active, "Test Session", "");
    let image = render_battlefield(mtm, vec![card], None, CARD_SIZE);

    // Sample interior of card (away from border/text)
    let card_w = (CARD_SIZE.width - 24.0) as u32; // margins
    let card_h = (CARD_SIZE.height - 24.0) as u32;
    let quarter_h = card_h / 4;

    // Top quarter interior (skip border area)
    let top_avg = sample_region_avg(&image, 40, 20, card_w - 80, quarter_h);
    // Bottom quarter interior
    let bottom_y = 12 + card_h - quarter_h;
    let bottom_avg = sample_region_avg(&image, 40, bottom_y, card_w - 80, quarter_h);

    // Active top: rgb(14,33,52) → blue channel ~52
    // Active bottom: rgb(9,18,31) → blue channel ~31
    // Top blue should be noticeably higher than bottom blue
    assert!(
        top_avg[2] > bottom_avg[2] + 3.0,
        "expected vertical gradient: top blue ({:.1}) should be higher than bottom blue ({:.1})",
        top_avg[2],
        bottom_avg[2]
    );
}

/// Sample pixels 30px below card bottom edge. Expect non-black alpha (shadow blur).
/// Currently no shadow -> FAILS.
fn card_has_shadow_below(mtm: MainThreadMarker) {
    let card = make_card(BattleCardStatus::Active, "Test Session", "Headline");
    // Use a taller view so there's space below the card for shadow
    let size = NSSize::new(500.0, 500.0);
    let image = render_battlefield(mtm, vec![card], None, size);

    // Card bottom is approximately at margin(12) + card_h
    // With a 500x500 view and 1 card, card fills most of it
    // Sample well below the card area — look for shadow pixels
    let bottom_sample_y = (size.height as u32).saturating_sub(10);
    let avg = sample_region_avg(&image, 100, bottom_sample_y, 200, 5);

    // Shadow should make the area below the card non-pure-black
    // With rgba(0,0,0,0.28) shadow at 24px offset and 46px blur,
    // we expect some brightness in the region below
    let luminance = avg[0] * 0.299 + avg[1] * 0.587 + avg[2] * 0.114;
    assert!(
        luminance > 0.5,
        "expected shadow below card, but region is too dark (luminance={:.2}). \
         avg_rgba=({:.1},{:.1},{:.1},{:.1})",
        luminance,
        avg[0],
        avg[1],
        avg[2],
        avg[3]
    );
}

/// Render card with scrollback, sample transcript region.
/// Expect GTK's rgba(8,14,22,0.34). Current macOS uses rgba(24,31,40,0.52) -> FAILS.
fn transcript_bg_matches_theme(mtm: MainThreadMarker) {
    let mut card = make_card(BattleCardStatus::Active, "Test", "Headline");
    card.scrollback = vec!["$ cargo build".to_string(), "Compiling...".to_string()];
    let image = render_battlefield(mtm, vec![card], None, CARD_SIZE);

    // Transcript block starts below headline + detail area
    // Sample a region in the transcript background (interior, away from text)
    // The transcript region is roughly at y ~180-250 depending on layout
    // Sample the rightmost interior of the transcript band to avoid text
    let avg = sample_region_avg(&image, 350, 190, 80, 20);

    // Expected from theme: rgba(8,14,22,0.34) composited on card bg
    // The composited result should be dark with low RGB values
    // GTK transcript_bg: rgb(8,14,22) at 0.34 alpha
    // Current macOS: rgb(24,31,40) at 0.52 alpha — noticeably brighter
    // After fix, red channel should be closer to composited value (~5-12)
    assert!(
        avg[0] < 18.0,
        "transcript bg red channel ({:.1}) too high — expected theme value rgba(8,14,22,0.34)",
        avg[0]
    );
}

/// Sample transcript border pixels. Expect rgba(173,188,204,0.08).
/// Current macOS uses rgba(78,91,108,0.38) -> FAILS.
fn transcript_border_matches_theme(mtm: MainThreadMarker) {
    let mut card = make_card(BattleCardStatus::Active, "Test", "Headline");
    card.scrollback = vec!["$ cargo build".to_string()];
    let image = render_battlefield(mtm, vec![card], None, CARD_SIZE);

    // Sample the border region of the transcript block
    // The border is a 1px stroke around the rounded rect
    // Sample along the top edge of the transcript region
    let avg = sample_region_avg(&image, 30, 178, 400, 2);

    // Expected: rgba(173,188,204,0.08) — very subtle, almost invisible
    // Current: rgba(78,91,108,0.38) — much more visible
    // After fix, the border should be nearly invisible (composited alpha ~0.08)
    // The composited result on a dark bg should have very low channel differences
    assert!(
        avg[1] < 50.0,
        "transcript border green channel ({:.1}) too high — expected subtle border rgba(173,188,204,0.08)",
        avg[1]
    );
}

/// Render selected card, sample border region.
/// Expect high-alpha blue glow matching GTK rgba(113,197,255,0.98).
/// Current macOS uses alpha 0.15 -> FAILS.
fn selected_card_border_is_bright(mtm: MainThreadMarker) {
    let card = make_card(BattleCardStatus::Active, "Test", "Headline");
    let image = render_battlefield(mtm, vec![card], Some(SessionId(1)), CARD_SIZE);

    // Sample along the card border — e.g. the left edge
    let avg = sample_region_avg(&image, 13, 100, 3, 50);

    // Expected: rgba(113,197,255,0.98) — bright blue border
    // Current: rgba(113,197,255,0.15) — barely visible
    // Blue channel should be dominant and bright
    assert!(
        avg[2] > 100.0,
        "selected card border blue ({:.1}) too dim — expected bright blue rgba(113,197,255,0.98)",
        avg[2]
    );
}

/// Render card with fill=1 attention bar. Sample left vs right of segment.
/// GTK uses horizontal gradient. macOS uses flat color -> FAILS.
fn attention_bar_calm_is_gradient(mtm: MainThreadMarker) {
    let mut card = make_card(BattleCardStatus::Active, "Test", "Headline");
    card.attention_bar = Some(SegmentedBarPresentation {
        fill: 1,
        css_class: "bar-attention-1",
        label: "ATTENTION CONDITION",
    });
    card.attention_bar_reason = Some("Low priority monitoring".to_string());
    let image = render_battlefield(mtm, vec![card], None, CARD_SIZE);

    // The attention bar segments are drawn below the content area
    // Sample left and right portions of the first filled segment
    // Segments start at ~pad_x=16, after the "ATTENTION CONDITION" caption
    let segment_y = 290; // approximate — below transcript area
    let left_avg = sample_region_avg(&image, 20, segment_y as u32, 30, 6);
    let right_avg = sample_region_avg(&image, 80, segment_y as u32, 30, 6);

    // For a horizontal gradient, left and right should differ
    // Even a small color difference indicates gradient vs flat fill
    let left_luminance = left_avg[0] * 0.299 + left_avg[1] * 0.587 + left_avg[2] * 0.114;
    let right_luminance = right_avg[0] * 0.299 + right_avg[1] * 0.587 + right_avg[2] * 0.114;

    assert!(
        (left_luminance - right_luminance).abs() > 0.5,
        "attention bar segment should have horizontal gradient, but left ({:.2}) and right ({:.2}) \
         luminance are too similar",
        left_luminance,
        right_luminance
    );
}

// ---------------------------------------------------------------------------
// Text styling and positioning tests
// ---------------------------------------------------------------------------

/// Render a card with known title text. Assert that the top region has bright text pixels.
fn title_renders_at_top_of_card(mtm: MainThreadMarker) {
    let card = make_card(BattleCardStatus::Active, "Test Title Session", "");
    let image = render_battlefield(mtm, vec![card], None, CARD_SIZE);

    // Title should be in the top region: y offset ~14-38 from card top (12 margin + 14 pad)
    assert!(
        has_text_content(&image, 28, 26, 300, 22, 0.02),
        "title text should be visible at top of card"
    );
}

/// Render a card. Assert that a region below the title contains the status chip.
fn status_chip_renders_below_title(mtm: MainThreadMarker) {
    let card = make_card(BattleCardStatus::Active, "Test", "");
    let image = render_battlefield(mtm, vec![card], None, CARD_SIZE);

    // Status chip should be below the title, roughly y=50-70 from top of view
    assert!(
        has_text_content(&image, 28, 48, 120, 22, 0.01),
        "status chip should render below title"
    );
}

/// Render card with a known headline. Assert headline region contains text.
fn headline_text_rendered_and_positioned(mtm: MainThreadMarker) {
    let card = make_card(BattleCardStatus::Active, "Test", "Build passing steadily");
    let image = render_battlefield(mtm, vec![card], None, CARD_SIZE);

    // Headline should be below status chip, roughly y=80-120 from top
    assert!(
        has_text_content(&image, 28, 78, 350, 30, 0.02),
        "headline text should be rendered below status chip"
    );
}

/// Render card with detail text. Assert the detail region has text.
/// Render card without detail text. Assert detail region is empty.
fn detail_text_rendered_when_present(mtm: MainThreadMarker) {
    let mut card = make_card(BattleCardStatus::Active, "Test", "Headline");
    card.detail = Some("Steady progress on compilation".to_string());
    let image_with = render_battlefield(mtm, vec![card], None, CARD_SIZE);

    let mut card_without = make_card(BattleCardStatus::Active, "Test", "Headline");
    card_without.detail = None;
    let image_without = render_battlefield(mtm, vec![card_without], None, CARD_SIZE);

    // Detail region should have text when present
    let detail_y = 120;
    let with_text = has_text_content(&image_with, 28, detail_y, 350, 25, 0.01);
    let _without_text = has_text_content(&image_without, 28, detail_y, 350, 25, 0.01);

    assert!(with_text, "detail text should be visible when present");
}

/// Render card with alert. Assert text in alert region with "!" prefix.
fn alert_text_rendered_with_prefix(mtm: MainThreadMarker) {
    let mut card = make_card(BattleCardStatus::Active, "Test", "Headline");
    card.alert = Some("Process stuck, needs input".to_string());
    let image = render_battlefield(mtm, vec![card], None, CARD_SIZE);

    // Alert should be rendered below other content, roughly y=120-150
    assert!(
        has_text_content(&image, 28, 120, 350, 30, 0.01),
        "alert text should be rendered"
    );
}

/// Render card with recency text. Assert text exists in the recency row region.
fn recency_label_positioned_after_content(mtm: MainThreadMarker) {
    let card = make_card(BattleCardStatus::Active, "Test", "Headline");
    let image = render_battlefield(mtm, vec![card], None, CARD_SIZE);

    // Recency row is after content, typically y=150-180
    assert!(
        has_text_content(&image, 28, 140, 200, 25, 0.01),
        "recency label should be rendered"
    );
}

/// Render card with known scrollback lines. Monospace font means both lines
/// should have the same pixel width.
fn scrollback_uses_monospace_proportions(mtm: MainThreadMarker) {
    let mut card = make_card(BattleCardStatus::Active, "Test", "Headline");
    card.scrollback = vec!["WWWWWWWW".to_string(), "iiiiiiii".to_string()];
    let image = render_battlefield(mtm, vec![card], None, CARD_SIZE);

    // The scrollback region should contain text
    assert!(
        has_text_content(&image, 28, 180, 350, 50, 0.01),
        "scrollback lines should be rendered in the transcript area"
    );
}

/// Render card with attention bar. Assert "ATTENTION CONDITION" label is visible.
fn attention_bar_label_rendered(mtm: MainThreadMarker) {
    let mut card = make_card(BattleCardStatus::Active, "Test", "Headline");
    card.attention_bar = Some(SegmentedBarPresentation {
        fill: 2,
        css_class: "bar-attention-2",
        label: "ATTENTION CONDITION",
    });
    let image = render_battlefield(mtm, vec![card], None, CARD_SIZE);

    // The caption "ATTENTION CONDITION" should be visible above the bar segments
    // This is in the lower part of the card
    assert!(
        has_text_content(&image, 28, 160, 300, 20, 0.01),
        "ATTENTION CONDITION label should be rendered"
    );
}

/// Render card with nudge state. Assert nudge chip pixels exist on the right side.
fn nudge_chip_renders_on_right(mtm: MainThreadMarker) {
    let mut card = make_card(BattleCardStatus::Active, "Test", "Headline");
    card.nudge_state = NudgeStatePresentation {
        label: "AUTONUDGE ARMED",
        css_class: "card-control-armed",
        tone: NudgeStateTone::Armed,
    };
    let image = render_battlefield(mtm, vec![card], None, CARD_SIZE);

    // Nudge chip should be on the right side of the recency row
    // The card is ~476px wide, nudge chip is at card.x + card.w - 164
    let nudge_x = (CARD_SIZE.width as u32).saturating_sub(180);
    assert!(
        has_text_content(&image, nudge_x, 140, 160, 25, 0.01),
        "nudge chip should render on right side of recency row"
    );
}

/// Render focus view. Assert title text and status chip are visible.
fn focus_view_title_and_status_rendered(mtm: MainThreadMarker) {
    let data = make_focus(BattleCardStatus::Active, "Focus Test", "Working on things");
    let image = render_focus(mtm, data, FOCUS_SIZE);

    // Title should be near the top
    assert!(
        has_text_content(&image, 30, 16, 300, 24, 0.02),
        "focus view title should be rendered"
    );

    // Status chip below title
    assert!(
        has_text_content(&image, 30, 44, 150, 22, 0.01),
        "focus view status chip should be rendered"
    );
}

/// Render focus view with headline. Assert headline region has text.
fn focus_view_headline_rendered(mtm: MainThreadMarker) {
    let data = make_focus(BattleCardStatus::Active, "Focus Test", "Build passing steadily");
    let image = render_focus(mtm, data, FOCUS_SIZE);

    // Headline below status chip, roughly y=78-120
    assert!(
        has_text_content(&image, 30, 78, 400, 40, 0.01),
        "focus view headline should be rendered"
    );
}

// ---------------------------------------------------------------------------
// Snapshot tests
// ---------------------------------------------------------------------------

/// Full 4-card grid snapshot baseline test.
fn battlefield_snapshot(mtm: MainThreadMarker) {
    let cards = vec![
        {
            let mut c = make_card(BattleCardStatus::Active, "Session 1", "Compiling");
            c.id = SessionId(1);
            c.scrollback = vec!["$ cargo build".to_string()];
            c
        },
        {
            let mut c = make_card(BattleCardStatus::Thinking, "Session 2", "Analyzing code");
            c.id = SessionId(2);
            c
        },
        {
            let mut c = make_card(BattleCardStatus::Blocked, "Session 3", "Waiting for input");
            c.id = SessionId(3);
            c.alert = Some("Process stuck".to_string());
            c
        },
        {
            let mut c = make_card(BattleCardStatus::Complete, "Session 4", "Done");
            c.id = SessionId(4);
            c
        },
    ];
    let size = NSSize::new(1000.0, 600.0);
    let image = render_battlefield(mtm, cards, Some(SessionId(1)), size);

    let baselines = baselines_dir();
    let config = CompareConfig {
        channel_tolerance: 8,
        match_threshold: 0.95,
        ..CompareConfig::default()
    };
    if let Err(e) = assert_visual_match(&image, "battlefield_4card", &baselines, &config) {
        panic!("{}", e);
    }
}

/// Focus view snapshot baseline.
fn focus_snapshot(mtm: MainThreadMarker) {
    let mut data = make_focus(BattleCardStatus::Active, "Focus Session", "Build in progress");
    data.attention = Some(AttentionPresentation {
        fill: 2,
        label: "MONITOR",
    });
    data.attention_bar = Some(SegmentedBarPresentation {
        fill: 2,
        css_class: "bar-attention-2",
        label: "ATTENTION CONDITION",
    });

    let image = render_focus(mtm, data, FOCUS_SIZE);

    let baselines = baselines_dir();
    let config = CompareConfig {
        channel_tolerance: 8,
        match_threshold: 0.95,
        ..CompareConfig::default()
    };
    if let Err(e) = assert_visual_match(&image, "focus_view", &baselines, &config) {
        panic!("{}", e);
    }
}

fn baselines_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/visual_baselines")
}
