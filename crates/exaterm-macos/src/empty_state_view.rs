use objc2::rc::Retained;
use objc2::{MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSAutoresizingMaskOptions, NSLineBreakMode, NSTextField, NSView,
};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};

use crate::style;

pub const EMPTY_STATE_TITLE: &str = "No Live Sessions Yet";
pub const EMPTY_STATE_BODY: &str = "Use Add Shell to start a real terminal-native agent or open an operator shell. Exaterm opens into an empty battlefield so the workspace begins with your own sessions.";

pub struct EmptyStateViews {
    pub container: Retained<NSView>,
    pub title: Retained<NSTextField>,
    pub body: Retained<NSTextField>,
}

pub fn build_empty_state(mtm: MainThreadMarker, frame: NSRect) -> EmptyStateViews {
    let container = NSView::initWithFrame(NSView::alloc(mtm), frame);
    container.setAutoresizingMask(
        NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewHeightSizable,
    );

    let title = make_label(
        mtm,
        EMPTY_STATE_TITLE,
        NSRect::new(NSPoint::new(120.0, 220.0), NSSize::new(520.0, 34.0)),
        Some(&style::font_from_spec(&exaterm_ui::theme::card_title_font())),
    );
    let body = make_label(
        mtm,
        EMPTY_STATE_BODY,
        NSRect::new(NSPoint::new(120.0, 268.0), NSSize::new(760.0, 96.0)),
        Some(&style::font_from_spec(
            &exaterm_ui::theme::card_detail_font(),
        )),
    );
    body.setAllowsDefaultTighteningForTruncation(false);
    body.setPreferredMaxLayoutWidth(760.0);
    body.setMaximumNumberOfLines(0);
    body.setUsesSingleLineMode(false);
    body.setLineBreakMode(NSLineBreakMode::ByWordWrapping);
    if let Some(cell) = body.cell() {
        cell.setWraps(true);
        cell.setUsesSingleLineMode(false);
        cell.setLineBreakMode(NSLineBreakMode::ByWordWrapping);
    }

    title.setAutoresizingMask(NSAutoresizingMaskOptions::ViewMaxYMargin);
    body.setAutoresizingMask(NSAutoresizingMaskOptions::ViewWidthSizable);

    container.addSubview(&title);
    container.addSubview(&body);

    EmptyStateViews {
        container,
        title,
        body,
    }
}

fn make_label(
    mtm: MainThreadMarker,
    text: &str,
    frame: NSRect,
    font: Option<&objc2_app_kit::NSFont>,
) -> Retained<NSTextField> {
    let label = NSTextField::labelWithString(&NSString::from_str(text), mtm);
    label.setFrame(frame);
    label.setEditable(false);
    label.setSelectable(false);
    label.setDrawsBackground(false);
    label.setBordered(false);
    label.setBezeled(false);
    label.setTextColor(Some(&style::color_to_nscolor(
        &exaterm_ui::theme::title_color(),
    )));
    label.setAlignment(objc2_app_kit::NSTextAlignment::Left);
    if let Some(font) = font {
        label.setFont(Some(font));
    }
    label
}
