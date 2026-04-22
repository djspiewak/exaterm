use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc;

use objc2::define_class;
use objc2::rc::Retained;
use objc2::{AnyThread, MainThreadOnly};
use objc2_app_kit::{
    NSAttributedStringNSStringDrawing, NSBezierPath, NSColor, NSGraphicsContext, NSShadow, NSView,
};
use objc2_foundation::{NSAttributedString, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString};

use crate::app_state::FocusRenderData;
use crate::terminal_view::TerminalRenderState;
use exaterm_types::model::SessionId;
use exaterm_types::proto::ClientMessage;
use exaterm_types::synthesis::CardCharBudget;
use exaterm_types::synthesis::truncate_with_ellipsis;
use exaterm_ui::layout::{focus_card_char_budget, focus_terminal_slot_rect, FOCUS_STATUS_BAR_HEIGHT};
use exaterm_ui::presentation::chrome_visibility;
use exaterm_ui::theme;
use exaterm_ui::theme::Color;

thread_local! {
    static FOCUS: RefCell<Option<FocusRenderData>> = const { RefCell::new(None) };
    static RENDER: RefCell<Option<Rc<TerminalRenderState>>> = RefCell::new(None);
    static FOCUS_BUDGET_SENDER: RefCell<Option<mpsc::Sender<ClientMessage>>> = const { RefCell::new(None) };
    static LAST_FOCUS_BUDGET: RefCell<Option<(SessionId, CardCharBudget)>> = const { RefCell::new(None) };
}

pub fn set_focus_data(data: Option<FocusRenderData>, render: Rc<TerminalRenderState>) {
    FOCUS.with(|slot| *slot.borrow_mut() = data);
    RENDER.with(|slot| *slot.borrow_mut() = Some(render));
}

pub fn set_focus_budget_sender(sender: mpsc::Sender<ClientMessage>) {
    FOCUS_BUDGET_SENDER.with(|slot| *slot.borrow_mut() = Some(sender));
}

define_class!(
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FocusView"]
    pub struct FocusView;

    unsafe impl NSObjectProtocol for FocusView {}

    impl FocusView {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty_rect: NSRect) {
            draw_focus(self.frame());
        }

        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            true
        }
    }
);

fn draw_focus(frame: NSRect) {
    let Some(render) = RENDER.with(|slot| slot.borrow().clone()) else {
        return;
    };
    let Some(data) = FOCUS.with(|slot| slot.borrow().clone()) else {
        return;
    };

    let bg = crate::style::color_to_nscolor(&theme::focus_background());
    bg.setFill();
    NSBezierPath::fillRect(frame);

    let card_rect = NSRect::new(
        NSPoint::new(12.0, 0.0),
        NSSize::new((frame.size.width - 24.0).max(0.0), frame.size.height),
    );

    // Dispatch the focus-panel budget to the daemon (deduped by value).
    let focus_budget = focus_card_char_budget(card_rect.size.width);
    let changed = LAST_FOCUS_BUDGET.with(|b| {
        let mut b = b.borrow_mut();
        let key = (data.id, focus_budget);
        if b.as_ref() != Some(&key) {
            *b = Some(key);
            true
        } else {
            false
        }
    });
    if changed {
        FOCUS_BUDGET_SENDER.with(|slot| {
            if let Some(sender) = slot.borrow().as_ref() {
                let _ = sender.send(ClientMessage::ReportCardBudget {
                    session_id: data.id,
                    budget: focus_budget,
                });
            }
        });
    }

    let corner = 24.0;
    let path = NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(card_rect, corner, corner);
    let visual_status = data.visual_status();
    let border = crate::style::color_to_nscolor(&theme::focus_card_border());
    // Draw shadow before fill.
    {
        let shadow_theme = theme::card_theme(visual_status).shadow;
        let shadow = NSShadow::new();
        shadow.setShadowOffset(NSSize::new(0.0, -f64::from(shadow_theme.offset_y)));
        shadow.setShadowBlurRadius(f64::from(shadow_theme.blur));
        shadow.setShadowColor(Some(&crate::style::color_to_nscolor(&shadow_theme.color)));
        NSGraphicsContext::saveGraphicsState_class();
        shadow.set();
        render.card_bg_top(visual_status).setFill();
        path.fill();
        NSGraphicsContext::restoreGraphicsState_class();
    }
    crate::style::draw_vertical_gradient(
        &path,
        render.card_bg_top(visual_status),
        render.card_bg_bottom(visual_status),
    );
    border.setStroke();
    path.setLineWidth(1.0);
    path.stroke();

    NSGraphicsContext::saveGraphicsState_class();
    path.addClip();

    let pad_x = card_rect.origin.x + 18.0;
    let mut y = card_rect.origin.y + 16.0;
    let chrome = chrome_visibility(data.summarized(), true, false);
    if chrome.title_visible {
        let title = truncate_with_ellipsis(&data.title, focus_budget.title_chars.into());
        build_simple_attr_string(&title, &render.title_font, &render.title_color)
            .drawAtPoint(NSPoint::new(pad_x, y));
        y += 28.0;
    }

    if chrome.status_visible {
        draw_chip(
            &data.status_label,
            render.chip_text_color(visual_status),
            render.chip_bg_color(visual_status),
            &render.status_font,
            pad_x,
            y,
        );
        if let Some(attention) = data.attention {
            draw_chip(
                attention.label,
                &render.attention_chip_text,
                render.attention_chip_bg(attention.fill),
                &render.status_font,
                pad_x + 140.0,
                y,
            );
        }
        y += 34.0;
    }

    if chrome.headline_visible && !data.combined_headline.is_empty() {
        let headline = truncate_with_ellipsis(&data.combined_headline, focus_budget.headline_chars.into());
        build_simple_attr_string(
            &headline,
            &render.headline_font,
            &render.headline_color,
        )
        .drawInRect(NSRect::new(
            NSPoint::new(pad_x, y),
            NSSize::new((card_rect.size.width - 36.0).max(0.0), 56.0),
        ));
    }

    let slot = focus_terminal_slot_rect(frame.size.width as i32, frame.size.height as i32);
    let slot_rect = NSRect::new(
        NSPoint::new(slot.x, slot.y),
        NSSize::new(slot.w.max(0.0), slot.h.max(0.0)),
    );
    let terminal_bg = ns_color(theme::focus_terminal_slot_bg());
    terminal_bg.setFill();
    let terminal_path =
        NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(slot_rect, 18.0, 18.0);
    terminal_path.fill();

    NSGraphicsContext::restoreGraphicsState_class();

    if !chrome.header_visible {
        return;
    }

    // Status bar at the very bottom of the focus view.
    let status_bar_y = frame.size.height - FOCUS_STATUS_BAR_HEIGHT;
    let status_bar_rect = NSRect::new(
        NSPoint::new(0.0, status_bar_y),
        NSSize::new(frame.size.width, FOCUS_STATUS_BAR_HEIGHT),
    );
    let bar_bg = crate::style::color_to_nscolor(&theme::status_bar_bg());
    bar_bg.setFill();
    NSBezierPath::fillRect(status_bar_rect);
    let status_text = format!("{} — {}", data.title, data.status_label);
    let bar_text_color = crate::style::color_to_nscolor(&theme::status_bar_text_color());
    let bar_font = crate::style::font_from_spec(&theme::card_recency_font());
    build_simple_attr_string(&status_text, &bar_font, &bar_text_color)
        .drawAtPoint(NSPoint::new(18.0, status_bar_y + 6.0));
}

fn draw_chip(
    label: &str,
    text: &Retained<NSColor>,
    bg: &Retained<NSColor>,
    font: &objc2_app_kit::NSFont,
    x: f64,
    y: f64,
) {
    let chip_w = label.len() as f64 * 7.4 + 18.0;
    let chip_rect = NSRect::new(NSPoint::new(x, y), NSSize::new(chip_w, 22.0));
    let chip_path = NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(chip_rect, 9.0, 9.0);
    bg.setFill();
    chip_path.fill();
    build_simple_attr_string(label, font, text).drawAtPoint(NSPoint::new(x + 9.0, y + 3.0));
}

fn ns_color(c: Color) -> Retained<NSColor> {
    NSColor::colorWithSRGBRed_green_blue_alpha(
        f64::from(c.r) / 255.0,
        f64::from(c.g) / 255.0,
        f64::from(c.b) / 255.0,
        f64::from(c.a),
    )
}

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
