// Custom NSView for rendering battlefield cards with Core Graphics.
//
// Uses thread-local storage to pass card data to the view's drawRect:
// implementation, avoiding complex objc2 define_class! ivars.

use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;
use std::rc::Rc;

use objc2::define_class;
use objc2::rc::Retained;
use objc2::{AnyThread, MainThreadOnly};
use objc2_app_kit::{
    NSAttributedStringNSStringDrawing, NSBezierPath, NSColor, NSEvent, NSGraphicsContext, NSShadow,
    NSView,
};
use objc2_foundation::{NSAttributedString, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString};

use crate::app_state::CardRenderData;
use crate::style;
use crate::terminal_view::TerminalRenderState;
use exaterm_types::model::SessionId;
use exaterm_types::proto::ClientMessage;
use exaterm_types::synthesis::CardCharBudget;
use exaterm_ui::layout::{
    card_char_budget, card_layout, card_terminal_slot_rect, focus_card_layout, CardRect, MARGIN,
};
use exaterm_ui::presentation::NudgeStateTone;
use exaterm_ui::presentation::{chrome_visibility, ChromeVisibility};
use std::collections::BTreeMap;
use std::sync::mpsc;

// ---------------------------------------------------------------------------
// Thread-local data bridge (main thread only)
// ---------------------------------------------------------------------------

thread_local! {
    static CARDS: RefCell<Vec<CardRenderData>> = const { RefCell::new(Vec::new()) };
    static SELECTED: Cell<Option<SessionId>> = const { Cell::new(None) };
    static RENDER: RefCell<Option<Rc<TerminalRenderState>>> = RefCell::new(None);
    static INTERACTION: RefCell<Option<Rc<dyn Fn(BattlefieldInteraction)>>> = RefCell::new(None);
    static EMBEDDED: RefCell<BTreeSet<SessionId>> = RefCell::new(BTreeSet::new());
    static FOCUSED: Cell<bool> = const { Cell::new(false) };
    static LAST_FOCUSED: Cell<bool> = const { Cell::new(false) };
    /// Last-sent card budget per session — used to dedup on every redraw.
    static LAST_SENT_BUDGETS: RefCell<BTreeMap<SessionId, CardCharBudget>> =
        RefCell::new(BTreeMap::new());
    static BUDGET_SENDER: RefCell<Option<mpsc::Sender<ClientMessage>>> = RefCell::new(None);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BattlefieldInteraction {
    Select(SessionId),
    Focus(SessionId),
}

/// Push new card data for the next drawRect: cycle.
pub fn set_battlefield_data(
    cards: Vec<CardRenderData>,
    selected: Option<SessionId>,
    render: Rc<TerminalRenderState>,
    embedded: BTreeSet<SessionId>,
    focused: bool,
) {
    CARDS.with(|c| *c.borrow_mut() = cards);
    SELECTED.with(|s| s.set(selected));
    RENDER.with(|r| *r.borrow_mut() = Some(render));
    EMBEDDED.with(|slot| *slot.borrow_mut() = embedded);
    FOCUSED.with(|slot| slot.set(focused));
}

pub fn set_interaction_handler<F>(handler: F)
where
    F: Fn(BattlefieldInteraction) + 'static,
{
    INTERACTION.with(|slot| *slot.borrow_mut() = Some(Rc::new(handler)));
}

/// Register the command sender so `draw_battlefield` can dispatch `ReportCardBudget`.
pub fn set_budget_sender(sender: mpsc::Sender<ClientMessage>) {
    BUDGET_SENDER.with(|slot| *slot.borrow_mut() = Some(sender));
}

// ---------------------------------------------------------------------------
// BattlefieldView — custom NSView subclass
// ---------------------------------------------------------------------------

define_class!(
    // SAFETY: NSView has no special subclassing requirements beyond drawRect:.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "BattlefieldView"]
    pub struct BattlefieldView;

    unsafe impl NSObjectProtocol for BattlefieldView {}

    impl BattlefieldView {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty_rect: NSRect) {
            draw_battlefield(self.frame());
        }

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, event: &NSEvent) {
            let point = self.convertPoint_fromView(event.locationInWindow(), None);
            if let Some(session_id) = session_at_point(self.frame(), point) {
                let interaction = if event.clickCount() >= 2 {
                    BattlefieldInteraction::Focus(session_id)
                } else {
                    BattlefieldInteraction::Select(session_id)
                };
                INTERACTION.with(|slot| {
                    if let Some(handler) = slot.borrow().as_ref() {
                        handler(interaction);
                    }
                });
                self.setNeedsDisplay(true);
            }
        }

        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            true
        }
    }
);

// ---------------------------------------------------------------------------
// Drawing — reads thread-locals and paints via Core Graphics
// ---------------------------------------------------------------------------

fn draw_battlefield(frame: NSRect) {
    let cards = CARDS.with(|c| c.borrow().clone());
    let selected = SELECTED.with(|s| s.get());
    let render = RENDER.with(|r| r.borrow().clone());
    let embedded = EMBEDDED.with(|slot| slot.borrow().clone());
    let focused = FOCUSED.with(|slot| slot.get());

    let last_focused = LAST_FOCUSED.with(|f| f.get());
    LAST_FOCUSED.with(|f| f.set(focused));
    if focused != last_focused {
        // Focus mode changed — invalidate the selected session's dedup entry so it
        // gets re-dispatched with the correct budget for the new mode.
        if let Some(focused_id) = selected {
            LAST_SENT_BUDGETS.with(|last| {
                last.borrow_mut().remove(&focused_id);
            });
        }
    }

    let render = match render {
        Some(r) => r,
        None => return,
    };

    if cards.is_empty() {
        // Draw fallback text.
        let text = NSString::from_str("Connecting to daemon...");
        let fallback = NSAttributedString::initWithString(NSAttributedString::alloc(), &text);
        fallback.drawAtPoint(NSPoint {
            x: MARGIN,
            y: MARGIN,
        });
        return;
    }

    let rects = layout_for_mode(cards.len(), frame, focused);

    let budgets: Vec<CardCharBudget> = rects.iter().map(|r| card_char_budget(r.w)).collect();

    // Dispatch ReportCardBudget for any card whose budget changed since last draw.
    BUDGET_SENDER.with(|sender_slot| {
        if let Some(sender) = sender_slot.borrow().as_ref() {
            LAST_SENT_BUDGETS.with(|last| {
                let mut last = last.borrow_mut();
                for ((card, _rect), &budget) in cards.iter().zip(rects.iter()).zip(budgets.iter()) {
                    // When in focus mode, skip the focused session — focus_view.rs
                    // dispatches the wider panel budget for that session.
                    if focused && selected == Some(card.id) {
                        continue;
                    }
                    if last.get(&card.id) != Some(&budget) {
                        last.insert(card.id, budget);
                        let _ = sender.send(ClientMessage::ReportCardBudget {
                            session_id: card.id,
                            budget,
                        });
                    }
                }
                last.retain(|id, _| cards.iter().any(|c| c.id == *id));
            });
        }
    });

    for ((card, rect), &live_budget) in cards.iter().zip(rects.iter()).zip(budgets.iter()) {
        let is_selected = selected == Some(card.id);
        draw_card(
            card,
            rect,
            is_selected,
            embedded.contains(&card.id),
            focused,
            &render,
            &live_budget,
        );
    }
}

fn session_at_point(frame: NSRect, point: NSPoint) -> Option<SessionId> {
    let cards = CARDS.with(|c| c.borrow().clone());
    let focused = FOCUSED.with(|slot| slot.get());
    let rects = layout_for_mode(cards.len(), frame, focused);
    cards
        .iter()
        .zip(rects.iter())
        .find(|(_, rect)| point_in_rect(point, rect))
        .map(|(card, _)| card.id)
}

pub(crate) fn layout_for_mode(card_count: usize, frame: NSRect, focused: bool) -> Vec<CardRect> {
    if focused {
        focus_card_layout(card_count, frame.size.width, frame.size.height)
    } else {
        card_layout(card_count, frame.size.width, frame.size.height)
    }
}

pub(crate) fn card_chrome_visibility(
    card: &CardRenderData,
    focused_mode: bool,
) -> ChromeVisibility {
    let summarized = !card.headline.is_empty()
        || card
            .detail
            .as_deref()
            .is_some_and(|detail| !detail.is_empty())
        || card.alert.as_deref().is_some_and(|alert| !alert.is_empty())
        || card.attention.is_some()
        || card.attention_bar.is_some();
    let has_operator_summary = card
        .last_nudge
        .as_deref()
        .is_some_and(|summary| !summary.is_empty());
    chrome_visibility(summarized, focused_mode, has_operator_summary)
}

fn point_in_rect(point: NSPoint, rect: &CardRect) -> bool {
    point.x >= rect.x
        && point.x <= rect.x + rect.w
        && point.y >= rect.y
        && point.y <= rect.y + rect.h
}

fn draw_card(
    card: &CardRenderData,
    rect: &CardRect,
    is_selected: bool,
    embedded_terminal: bool,
    focused_mode: bool,
    render: &TerminalRenderState,
    budget: &CardCharBudget,
) {
    use exaterm_types::synthesis::truncate_with_ellipsis;
    let ns_rect = NSRect::new(
        NSPoint {
            x: rect.x,
            y: rect.y,
        },
        NSSize {
            width: rect.w,
            height: rect.h,
        },
    );

    // Card background — rounded rect with shadow and vertical gradient fill.
    let layer = style::card_layer_style(card.status);
    let corner = layer.corner_radius;
    let path = NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(ns_rect, corner, corner);

    // Draw shadow before fill.
    {
        let shadow_theme = exaterm_ui::theme::card_theme(card.status).shadow;
        let shadow = NSShadow::new();
        shadow.setShadowOffset(objc2_foundation::NSSize::new(
            0.0,
            -f64::from(shadow_theme.offset_y), // negative Y for flipped view
        ));
        shadow.setShadowBlurRadius(f64::from(shadow_theme.blur));
        shadow.setShadowColor(Some(&style::color_to_nscolor(&shadow_theme.color)));
        NSGraphicsContext::saveGraphicsState_class();
        shadow.set();
        render.card_bg_top(card.status).setFill();
        path.fill();
        NSGraphicsContext::restoreGraphicsState_class();
    }

    style::draw_vertical_gradient(
        &path,
        render.card_bg_top(card.status),
        render.card_bg_bottom(card.status),
    );

    // Card border.
    let bc = &layer.border_color;
    let border_color = NSColor::colorWithSRGBRed_green_blue_alpha(bc.r, bc.g, bc.b, bc.a);
    border_color.setStroke();
    path.setLineWidth(1.0);
    path.stroke();

    // Selected card highlight.
    if is_selected {
        render.selected_bg.setStroke();
        path.setLineWidth(2.0);
        path.stroke();
    }

    // Clip to card bounds so text cannot overflow the rounded rect.
    NSGraphicsContext::saveGraphicsState_class();
    path.addClip();

    // --- Text content ---
    let pad_x = 16.0;
    let pad_y = 14.0;
    let mut y_cursor = rect.y + pad_y;
    let content_width = rect.w - 32.0;
    let chrome = card_chrome_visibility(card, focused_mode);

    let header_right_edge = rect.x + rect.w - pad_x;

    // Row 1: Title (left) + Status chip (right-anchored, same row).
    if chrome.title_visible {
        let title = truncate_with_ellipsis(&card.title, budget.title_chars.into());

        if chrome.status_visible {
            let chip_w = card.status_label.len() as f64 * 7.0 + 16.0;
            let chip_x = header_right_edge - chip_w;
            let mut status_y = y_cursor;
            draw_status_chip(&card.status_label, card.status, chip_x, &mut status_y, render);
        }

        let title_max_w = if chrome.status_visible {
            let chip_w = card.status_label.len() as f64 * 7.0 + 16.0;
            (header_right_edge - chip_w - 8.0 - (rect.x + pad_x)).max(0.0)
        } else {
            content_width
        };
        let title_str = build_simple_attr_string(&title, &render.title_font, &render.title_color);
        title_str.drawInRect(NSRect::new(
            NSPoint::new(rect.x + pad_x, y_cursor),
            NSSize::new(title_max_w, 22.0),
        ));
        y_cursor += if focused_mode { 20.0 } else { 24.0 };
    }

    // Row 2: Subtitle/concise headline (left) + Nudge chip (right-anchored, same row).
    // Uses card.headline only — the long attention_brief lives below the TTY.
    let headline = &card.headline;
    if chrome.headline_visible && !headline.is_empty() {
        let headline_clamped = truncate_with_ellipsis(headline, budget.headline_chars.into());

        if chrome.nudge_state_visible {
            let nudge_w = card.nudge_state.label.len() as f64 * 6.9 + 18.0;
            let nudge_x = header_right_edge - nudge_w;
            draw_nudge_chip(
                card.nudge_state.label,
                card.nudge_state.tone,
                nudge_x,
                y_cursor - 2.0,
                render,
            );
        }

        let subtitle_max_w = if chrome.nudge_state_visible {
            let nudge_w = card.nudge_state.label.len() as f64 * 6.9 + 18.0;
            (header_right_edge - nudge_w - 8.0 - (rect.x + pad_x)).max(0.0)
        } else {
            content_width
        };
        let subtitle_str =
            build_simple_attr_string(&headline_clamped, &render.subtitle_font, &render.subtitle_color);
        subtitle_str.drawInRect(NSRect::new(
            NSPoint::new(rect.x + pad_x, y_cursor),
            NSSize::new(subtitle_max_w, 20.0),
        ));
        y_cursor += 24.0;
    } else if focused_mode {
        y_cursor += 4.0;
    }

    if embedded_terminal {
        let slot = card_terminal_slot_rect(rect);
        let slot_rect = NSRect::new(NSPoint::new(slot.x, slot.y), NSSize::new(slot.w, slot.h));
        let terminal_path =
            NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(slot_rect, 16.0, 16.0);
        let terminal_bg = NSColor::colorWithSRGBRed_green_blue_alpha(0.02, 0.04, 0.07, 0.92);
        terminal_bg.setFill();
        terminal_path.fill();
        let label =
            build_simple_attr_string("LIVE TERMINAL", &render.recency_font, &render.recency_color);
        label.drawAtPoint(NSPoint::new(slot.x + 10.0, slot.y + 8.0));
        if chrome.bars_visible {
            if let Some(attention_bar) = card.attention_bar {
                draw_attention_condition_bar(
                    rect.x + pad_x,
                    (slot.y - 52.0).max(y_cursor),
                    content_width,
                    attention_bar.fill,
                    card.attention_bar_reason.as_deref(),
                    render,
                );
            }
        }
        NSGraphicsContext::restoreGraphicsState_class();
        return;
    }

    let raw_scrollback = scrollback_lines(card);
    if !raw_scrollback.is_empty() {
        let scrollback_lines =
            wrap_lines_to_fit(&raw_scrollback, content_width, render, 8);
        if !scrollback_lines.is_empty() {
            let scrollback_height = (scrollback_lines.len() as f64 * 18.0) + 16.0;
            draw_scrollback_band(
                rect.x + pad_x,
                y_cursor,
                content_width,
                scrollback_height,
                &scrollback_lines,
                render,
            );
            y_cursor += scrollback_height + 10.0;
        }
    }

    if chrome.bars_visible {
        if let Some(attention_bar) = card.attention_bar {
            draw_attention_condition_bar(
                rect.x + pad_x,
                y_cursor,
                content_width,
                attention_bar.fill,
                card.attention_bar_reason.as_deref(),
                render,
            );
        }
    }

    NSGraphicsContext::restoreGraphicsState_class();
}

fn draw_status_chip(
    label: &str,
    status: exaterm_ui::supervision::BattleCardStatus,
    x: f64,
    y_cursor: &mut f64,
    render: &TerminalRenderState,
) {
    let chip_text = render.chip_text_color(status);
    let chip_bg = render.chip_bg_color(status);

    // Approximate chip width from label length.
    let chip_w = label.len() as f64 * 7.0 + 16.0;
    let chip_h = 20.0;
    let chip_rect = NSRect::new(
        NSPoint { x, y: *y_cursor },
        NSSize {
            width: chip_w,
            height: chip_h,
        },
    );
    let chip_path = NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(chip_rect, 8.0, 8.0);
    chip_bg.setFill();
    chip_path.fill();

    let chip_str = build_simple_attr_string(label, &render.status_font, chip_text);
    chip_str.drawAtPoint(NSPoint {
        x: x + 8.0,
        y: *y_cursor + 2.0,
    });
    *y_cursor += chip_h + 4.0;
}


fn draw_nudge_chip(
    label: &str,
    tone: NudgeStateTone,
    x: f64,
    y: f64,
    render: &TerminalRenderState,
) {
    let chip_w = label.len() as f64 * 6.9 + 18.0;
    let chip_rect = NSRect::new(NSPoint::new(x, y), NSSize::new(chip_w, 22.0));
    let chip_path = NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(chip_rect, 10.0, 10.0);
    render.nudge_bg_color(tone).setFill();
    chip_path.fill();
    let chip_str =
        build_simple_attr_string(label, &render.status_font, render.nudge_text_color(tone));
    chip_str.drawAtPoint(NSPoint::new(x + 9.0, y + 3.0));
}

/// Wrap `lines` so no display line exceeds `available_width` pixels.
///
/// Uses the font's `maximumAdvancement.width` (monospace: all chars identical)
/// to compute how many characters fit per line.  Lines shorter than the limit
/// are passed through unchanged; longer lines are split at character boundaries.
/// The total number of display lines is capped at `max_display_lines`.
fn wrap_lines_to_fit(
    lines: &[String],
    available_width: f64,
    render: &TerminalRenderState,
    max_display_lines: usize,
) -> Vec<String> {
    const H_PADDING: f64 = 20.0; // 10px each side inside the band
    let char_width = render.scrollback_font.maximumAdvancement().width.max(1.0);
    let chars_per_line = ((available_width - H_PADDING) / char_width).floor() as usize;
    if chars_per_line == 0 {
        return Vec::new();
    }
    let mut result = Vec::new();
    'outer: for line in lines {
        let trimmed = line.trim_end();
        if trimmed.chars().count() <= chars_per_line {
            result.push(trimmed.to_string());
        } else {
            let mut remaining = trimmed;
            while !remaining.is_empty() {
                if result.len() >= max_display_lines {
                    break 'outer;
                }
                let split_at = remaining
                    .char_indices()
                    .nth(chars_per_line)
                    .map(|(i, _)| i)
                    .unwrap_or(remaining.len());
                result.push(remaining[..split_at].to_string());
                remaining = &remaining[split_at..];
            }
        }
        if result.len() >= max_display_lines {
            break;
        }
    }
    result
}

fn draw_scrollback_band(
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    lines: &[String],
    render: &TerminalRenderState,
) {
    let rect = NSRect::new(NSPoint::new(x, y), NSSize::new(width, height));
    let path = NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(rect, 12.0, 12.0);
    render.scrollback_bg.setFill();
    path.fill();
    render.scrollback_border.setStroke();
    path.setLineWidth(1.0);
    path.stroke();

    let mut line_y = y + 10.0;
    for line in lines {
        let line_str =
            build_simple_attr_string(line, &render.scrollback_font, &render.scrollback_color);
        line_str.drawAtPoint(NSPoint::new(x + 10.0, line_y));
        line_y += 18.0;
    }
}

fn draw_attention_condition_bar(
    x: f64,
    y: f64,
    width: f64,
    fill: usize,
    reason: Option<&str>,
    render: &TerminalRenderState,
) {
    let caption = build_simple_attr_string(
        "ATTENTION CONDITION",
        &render.bar_caption_font,
        &render.bar_caption_color,
    );
    caption.drawAtPoint(NSPoint::new(x, y));

    let segment_y = y + 18.0;
    let gap = 4.0;
    let segment_width = ((width - (gap * 4.0)).max(0.0)) / 5.0;
    for index in 0..5 {
        let segment_x = x + (index as f64 * (segment_width + gap));
        let rect = NSRect::new(
            NSPoint::new(segment_x, segment_y),
            NSSize::new(segment_width, 8.0),
        );
        let path = NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(rect, 4.0, 4.0);
        if index < fill {
            let (left, right) = render.attention_bar_gradient(fill);
            style::draw_horizontal_gradient(&path, left, right);
        } else {
            render.bar_empty.setFill();
            path.fill();
        }
    }

    if let Some(reason) = reason {
        if !reason.is_empty() {
            let reason_str =
                build_simple_attr_string(reason, &render.bar_reason_font, &render.bar_reason_color);
            reason_str.drawInRect(NSRect::new(
                NSPoint::new(x, segment_y + 14.0),
                NSSize::new(width, 42.0),
            ));
        }
    }
}

pub(crate) fn scrollback_lines(card: &CardRenderData) -> Vec<String> {
    let mut lines = Vec::new();
    if let Some(nudge) = card.last_nudge.as_deref() {
        if !nudge.is_empty() {
            lines.push(format!("Nudge: {nudge}"));
        }
    }
    lines.extend(card.scrollback.iter().take(4).cloned());
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn point_in_rect_accepts_interior_point() {
        let rect = CardRect {
            x: 10.0,
            y: 20.0,
            w: 100.0,
            h: 80.0,
        };
        assert!(point_in_rect(NSPoint::new(50.0, 60.0), &rect));
    }

    #[test]
    fn point_in_rect_rejects_exterior_point() {
        let rect = CardRect {
            x: 10.0,
            y: 20.0,
            w: 100.0,
            h: 80.0,
        };
        assert!(!point_in_rect(NSPoint::new(5.0, 60.0), &rect));
        assert!(!point_in_rect(NSPoint::new(50.0, 105.0), &rect));
    }
}

/// Build an NSAttributedString with a single font + color.
fn build_simple_attr_string(
    text: &str,
    font: &objc2_app_kit::NSFont,
    color: &Retained<NSColor>,
) -> Retained<NSAttributedString> {
    use objc2::runtime::AnyObject;
    use objc2_app_kit::{NSFontAttributeName, NSForegroundColorAttributeName};
    use objc2_foundation::{NSMutableAttributedString, NSRange};

    let ns_text = NSString::from_str(text);
    let result = NSMutableAttributedString::new();
    let plain = NSAttributedString::initWithString(NSAttributedString::alloc(), &ns_text);
    result.appendAttributedString(&plain);

    let range = NSRange::new(0, result.length());
    unsafe {
        let font_key: &objc2_foundation::NSAttributedStringKey = NSFontAttributeName;
        let fg_key: &objc2_foundation::NSAttributedStringKey = NSForegroundColorAttributeName;
        result.addAttribute_value_range(font_key, font as &AnyObject, range);
        result.addAttribute_value_range(fg_key, &**color as &AnyObject, range);
    }

    Retained::into_super(result)
}
