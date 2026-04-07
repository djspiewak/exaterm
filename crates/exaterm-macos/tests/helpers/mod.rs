//! Test helper utilities for visual tests.

use std::rc::Rc;

use objc2::{msg_send, MainThreadOnly};
use objc2::rc::Retained;
use objc2_app_kit::{NSBackingStoreType, NSWindow, NSWindowStyleMask};
use objc2_foundation::{MainThreadMarker, NSPoint, NSRect, NSSize};

use exaterm_macos::app_state::{CardRenderData, FocusRenderData};
use exaterm_macos::battlefield_view;
use exaterm_macos::focus_view;
use exaterm_macos::terminal_view::TerminalRenderState;
use exaterm_test_util::capture;
use exaterm_test_util::pixel_compare::RgbaImage;
use exaterm_types::model::SessionId;
use exaterm_ui::presentation::{NudgeStatePresentation, NudgeStateTone};
use exaterm_ui::supervision::BattleCardStatus;

/// Build a `CardRenderData` with sensible defaults.
pub fn make_card(status: BattleCardStatus, title: &str, headline: &str) -> CardRenderData {
    CardRenderData {
        id: SessionId(1),
        title: title.to_string(),
        status,
        status_label: status.label().to_string(),
        recency: "just now".to_string(),
        scrollback: Vec::new(),
        headline: headline.to_string(),
        combined_headline: headline.to_string(),
        detail: None,
        alert: None,
        attention: None,
        attention_reason: None,
        attention_bar: None,
        attention_bar_reason: None,
        nudge_state: NudgeStatePresentation {
            label: "AUTONUDGE OFF",
            css_class: "card-control-off",
            tone: NudgeStateTone::Off,
        },
        last_nudge: None,
    }
}

/// Build a `FocusRenderData` with sensible defaults.
pub fn make_focus(status: BattleCardStatus, title: &str, headline: &str) -> FocusRenderData {
    FocusRenderData {
        id: SessionId(1),
        title: title.to_string(),
        status,
        status_label: status.label().to_string(),
        combined_headline: headline.to_string(),
        attention: None,
        attention_reason: None,
        attention_bar: None,
        attention_bar_reason: None,
    }
}

/// Create an NSWindow + BattlefieldView, set card data, capture off-screen, return RgbaImage.
pub fn render_battlefield(
    mtm: MainThreadMarker,
    cards: Vec<CardRenderData>,
    selected: Option<SessionId>,
    size: NSSize,
) -> RgbaImage {
    let render = Rc::new(TerminalRenderState::new());
    let embedded = std::collections::BTreeSet::new();

    battlefield_view::set_battlefield_data(cards, selected, render, embedded, false);

    let window = create_test_window(mtm, size);
    let view: Retained<battlefield_view::BattlefieldView> = unsafe {
        let frame = NSRect::new(NSPoint::new(0.0, 0.0), size);
        msg_send![battlefield_view::BattlefieldView::alloc(mtm), initWithFrame: frame]
    };
    window.setContentView(Some(&view));
    window.display();

    exaterm_test_util::appkit_harness::flush_runloop();

    let captured = capture::capture_view(&view).expect("failed to capture battlefield view");
    captured.into()
}

/// Create an NSWindow + FocusView, set focus data, capture off-screen, return RgbaImage.
pub fn render_focus(
    mtm: MainThreadMarker,
    data: FocusRenderData,
    size: NSSize,
) -> RgbaImage {
    let render = Rc::new(TerminalRenderState::new());

    focus_view::set_focus_data(Some(data), render);

    let window = create_test_window(mtm, size);
    let view: Retained<focus_view::FocusView> = unsafe {
        let frame = NSRect::new(NSPoint::new(0.0, 0.0), size);
        msg_send![focus_view::FocusView::alloc(mtm), initWithFrame: frame]
    };
    window.setContentView(Some(&view));
    window.display();

    exaterm_test_util::appkit_harness::flush_runloop();

    let captured = capture::capture_view(&view).expect("failed to capture focus view");
    captured.into()
}

fn create_test_window(mtm: MainThreadMarker, size: NSSize) -> Retained<NSWindow> {
    let style = NSWindowStyleMask::Titled
        | NSWindowStyleMask::Closable
        | NSWindowStyleMask::Resizable;
    let rect = NSRect::new(NSPoint::new(100.0, 100.0), size);
    unsafe {
        NSWindow::initWithContentRect_styleMask_backing_defer(
            NSWindow::alloc(mtm),
            rect,
            style,
            NSBackingStoreType::Buffered,
            false,
        )
    }
}

/// Average RGBA across a rectangular region of an image.
///
/// Returns [avg_r, avg_g, avg_b, avg_a] where channels are 0.0–255.0.
#[allow(clippy::cast_lossless)]
pub fn sample_region_avg(image: &RgbaImage, x: u32, y: u32, w: u32, h: u32) -> [f64; 4] {
    let mut sum = [0.0f64; 4];
    let mut count = 0u64;
    for py in y..y.saturating_add(h).min(image.height) {
        for px in x..x.saturating_add(w).min(image.width) {
            if let Some(pixel) = exaterm_test_util::pixel_compare::pixel_at(image, px, py) {
                sum[0] += pixel[0] as f64;
                sum[1] += pixel[1] as f64;
                sum[2] += pixel[2] as f64;
                sum[3] += pixel[3] as f64;
                count += 1;
            }
        }
    }
    if count == 0 {
        return [0.0; 4];
    }
    [
        sum[0] / count as f64,
        sum[1] / count as f64,
        sum[2] / count as f64,
        sum[3] / count as f64,
    ]
}

/// Check if a region contains non-background pixels (text rendered).
///
/// Returns true if more than `bg_threshold` fraction of pixels have
/// luminance above 0.15 (indicating text or content).
#[allow(clippy::cast_lossless)]
pub fn has_text_content(
    image: &RgbaImage,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
    bg_threshold: f64,
) -> bool {
    let sub = crop_region(image, x, y, w, h);
    let bright = exaterm_test_util::pixel_compare::bright_pixel_fraction(&sub, 0.15);
    bright > bg_threshold
}

/// Crop a sub-region of an RgbaImage.
fn crop_region(image: &RgbaImage, x: u32, y: u32, w: u32, h: u32) -> RgbaImage {
    let x_end = x.saturating_add(w).min(image.width);
    let y_end = y.saturating_add(h).min(image.height);
    let x_start = x.min(x_end);
    let y_start = y.min(y_end);
    let actual_w = x_end - x_start;
    let actual_h = y_end - y_start;
    let mut data = Vec::with_capacity((actual_w * actual_h * 4) as usize);
    for py in y_start..y_end {
        for px in x_start..x_end {
            if let Some(pixel) = exaterm_test_util::pixel_compare::pixel_at(image, px, py) {
                data.extend_from_slice(&pixel);
            }
        }
    }
    RgbaImage::new(actual_w, actual_h, data)
}

/// Standard card render size for tests.
pub const CARD_SIZE: NSSize = NSSize::new(500.0, 400.0);

/// Standard focus render size for tests.
pub const FOCUS_SIZE: NSSize = NSSize::new(800.0, 600.0);
