//! Safe wrappers for capturing AppKit view contents as pixel data.
//!
//! Uses `NSView.cacheDisplay(in:toBitmapImageRep:)` to render a view's
//! layer tree into an `NSBitmapImageRep`, then extracts the raw RGBA
//! bytes. This approach works without screen recording permissions and
//! does not require the window to be visible on screen.

use std::ffi::c_uchar;
use std::ptr;

use objc2::{AnyThread, ClassType};
use objc2_app_kit::{NSBitmapImageRep, NSClipView, NSSplitView, NSView, NSWindow};
use objc2_foundation::{NSObjectProtocol, NSPoint, NSRect, NSSize, NSString};

use crate::pixel_compare::RgbaImage;

/// Raw RGBA pixel capture from an AppKit view.
///
/// Row-major, 4 bytes per pixel (R, G, B, A).
#[derive(Clone, Debug)]
pub struct CapturedImage {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

impl From<CapturedImage> for RgbaImage {
    fn from(img: CapturedImage) -> Self {
        RgbaImage {
            width: img.width,
            height: img.height,
            data: img.data,
        }
    }
}

/// Capture the rendered contents of an `NSView` as raw RGBA pixels.
///
/// The view must be part of a window, but the window need not be visible
/// on screen. Returns `None` if the view has zero area or the capture fails.
#[must_use]
pub fn capture_view(view: &NSView) -> Option<CapturedImage> {
    if let Some(window) = view.window() {
        ensure_window_has_frame(&window);
        window.display();
    }

    let rect = effective_capture_rect(view);
    capture_rect(view, rect)
}

const MIN_CAPTURE_DIM: f64 = 50.0;

const DEFAULT_CAPTURE_FRAME: NSRect = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(800.0, 600.0));

/// Capture the entire content view of an `NSWindow`.
#[must_use]
pub fn capture_window(window: &NSWindow) -> Option<CapturedImage> {
    let content = window.contentView()?;
    capture_view(&content)
}

/// Capture a sub-region of a view (in the view's coordinate system).
#[must_use]
pub fn capture_view_rect(view: &NSView, rect: NSRect) -> Option<CapturedImage> {
    capture_rect(view, rect)
}

// ---------------------------------------------------------------------------
// Internal
// ---------------------------------------------------------------------------

fn ensure_window_has_frame(window: &NSWindow) {
    let frame = window.frame();
    if frame.size.width < MIN_CAPTURE_DIM || frame.size.height < MIN_CAPTURE_DIM {
        window.setMinSize(DEFAULT_CAPTURE_FRAME.size);
        window.setFrame_display(DEFAULT_CAPTURE_FRAME, true);
        if let Some(content) = window.contentView() {
            force_split_view_layout(&content);
        }
    }
}

fn force_split_view_layout(view: &NSView) {
    if view.isKindOfClass(NSSplitView::class()) {
        // SAFETY: We confirmed the view is an NSSplitView via isKindOfClass.
        let split: &NSSplitView = unsafe { &*(ptr::from_ref(view).cast()) };
        split.adjustSubviews();
    }
    for subview in view.subviews().to_vec() {
        force_split_view_layout(&subview);
    }
}

fn effective_capture_rect(view: &NSView) -> NSRect {
    let clip_view = unsafe { view.superview() };
    let is_document_view = clip_view
        .as_ref()
        .is_some_and(|cv| cv.isKindOfClass(NSClipView::class()));

    if is_document_view {
        let scroll_view = clip_view.and_then(|cv| unsafe { cv.superview() });
        if let Some(sv) = &scroll_view {
            let sv_frame = sv.frame();
            if sv_frame.size.width >= MIN_CAPTURE_DIM && sv_frame.size.height >= MIN_CAPTURE_DIM {
                return NSRect::new(NSPoint::new(0.0, 0.0), sv_frame.size);
            }
        }
        if let Some(window) = view.window() {
            let frame = window.frame();
            if frame.size.width >= MIN_CAPTURE_DIM && frame.size.height >= MIN_CAPTURE_DIM {
                return NSRect::new(NSPoint::new(0.0, 0.0), frame.size);
            }
        }
    }

    view.bounds()
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn capture_rect(view: &NSView, rect: NSRect) -> Option<CapturedImage> {
    let width = rect.size.width as u32;
    let height = rect.size.height as u32;

    if width == 0 || height == 0 {
        return None;
    }

    view.display();

    let bitmap_rep = create_bitmap_rep(width, height)?;
    view.cacheDisplayInRect_toBitmapImageRep(rect, &bitmap_rep);

    extract_pixels(&bitmap_rep, width, height)
}

#[allow(clippy::cast_possible_wrap)]
fn create_bitmap_rep(width: u32, height: u32) -> Option<objc2::rc::Retained<NSBitmapImageRep>> {
    let color_space = NSString::from_str("NSDeviceRGBColorSpace");
    let w = width as isize;
    let h = height as isize;
    let bytes_per_row = w.checked_mul(4)?;

    // SAFETY: We pass null for planes (asking AppKit to allocate the
    // backing store), and all integer parameters are in valid ranges.
    unsafe {
        NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bytesPerRow_bitsPerPixel(
            NSBitmapImageRep::alloc(),
            ptr::null_mut::<*mut c_uchar>(),
            w,
            h,
            8,
            4,
            true,
            false,
            &color_space,
            bytes_per_row,
            32,
        )
    }
}

#[allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]
fn extract_pixels(
    rep: &NSBitmapImageRep,
    width: u32,
    height: u32,
) -> Option<CapturedImage> {
    let ptr = rep.bitmapData();
    if ptr.is_null() {
        return None;
    }

    let row_bytes = width as usize * 4;
    let bytes_per_row = rep.bytesPerRow() as usize;

    let data = if bytes_per_row == row_bytes {
        let byte_count = row_bytes.checked_mul(height as usize)?;
        unsafe { std::slice::from_raw_parts(ptr, byte_count) }.to_vec()
    } else {
        let mut data = Vec::with_capacity(row_bytes * height as usize);
        for y in 0..height as usize {
            let offset = y * bytes_per_row;
            let row = unsafe { std::slice::from_raw_parts(ptr.add(offset), row_bytes) };
            data.extend_from_slice(row);
        }
        data
    };

    Some(CapturedImage { width, height, data })
}
