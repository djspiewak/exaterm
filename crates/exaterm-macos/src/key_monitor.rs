#[cfg(target_os = "macos")]
mod imp {
    use std::cell::RefCell;
    use std::collections::BTreeMap;
    use std::rc::Rc;

    use objc2::rc::Retained;
    use objc2::runtime::AnyObject;
    use objc2_app_kit::{NSApplication, NSEvent, NSEventMask, NSEventModifierFlags, NSView, NSWindow};
    use objc2_foundation::{MainThreadMarker, NSRect};

    use exaterm_types::model::SessionId;

    use crate::app_state::AppState;
    use crate::key_dispatch;
    use crate::workspace_support;

    /// RAII handle that removes the NSEvent local monitor on drop.
    pub struct KeyMonitorHandle {
        monitor: Option<Retained<AnyObject>>,
    }

    impl Drop for KeyMonitorHandle {
        fn drop(&mut self) {
            if let Some(m) = self.monitor.take() {
                unsafe { NSEvent::removeMonitor(&m) };
            }
        }
    }

    /// Registers an AppKit local event monitor that dispatches keyboard events for
    /// an embedded battlefield terminal.
    ///
    /// The monitor intercepts `KeyDown` events for the given `window`. It handles
    /// focus-mode transitions, embedded-terminal pass-through, and session navigation.
    /// `on_add_terminal` is called when Cmd+N is pressed.
    ///
    /// AppKit copies the block internally, so the returned `KeyMonitorHandle` is the
    /// only thing the caller must keep alive: dropping it removes the monitor.
    pub fn register_battlefield_key_monitor(
        state: Rc<RefCell<AppState>>,
        window: Retained<NSWindow>,
        surfaces: Rc<RefCell<BTreeMap<SessionId, Retained<NSView>>>>,
        on_add_terminal: impl Fn() + 'static,
    ) -> KeyMonitorHandle {
        let on_add_terminal = Rc::new(on_add_terminal);
        let key_block = block2::StackBlock::new(
            move |event: std::ptr::NonNull<objc2_app_kit::NSEvent>| -> *mut objc2_app_kit::NSEvent {
                let event_ref = unsafe { event.as_ref() };

                let mtm = MainThreadMarker::new().expect("main thread");
                if NSApplication::sharedApplication(mtm)
                    .modalWindow()
                    .is_some()
                {
                    return event.as_ptr();
                }

                let key_code = event_ref.keyCode();
                let flags = event_ref.modifierFlags();
                let command = flags.contains(NSEventModifierFlags::Command);
                let in_focus = state.borrow().workspace.focused_session().is_some();

                if command && key_code == 45 {
                    on_add_terminal();
                    return std::ptr::null_mut();
                }
                if command {
                    return event.as_ptr();
                }

                if !in_focus {
                    let selected_embedded = window.contentView().is_some_and(|content| {
                        battlefield_embeds_selected_terminal(&state.borrow(), content.frame())
                    });
                    if selected_embedded {
                        let selected = state.borrow().workspace.selected_session();
                        if let Some(session_id) = selected {
                            if let Some(surface) = surfaces.borrow().get(&session_id) {
                                if key_code == 36 {
                                    window.makeFirstResponder(Some(&**surface));
                                }
                                return match key_dispatch::embedded_terminal_key_action(key_code) {
                                    key_dispatch::KeyAction::PassThrough => event.as_ptr(),
                                    key_dispatch::KeyAction::Consume => std::ptr::null_mut(),
                                };
                            }
                        }
                    }

                    match key_code {
                        36 => {
                            let selected = state.borrow().workspace.selected_session();
                            if let Some(session_id) = selected {
                                state
                                    .borrow_mut()
                                    .workspace
                                    .enter_focus_mode(session_id);
                                if let Some(surface) = surfaces.borrow().get(&session_id) {
                                    window.makeFirstResponder(Some(&**surface));
                                }
                            }
                            return std::ptr::null_mut();
                        }
                        126 => {
                            state.borrow_mut().select_previous_session();
                            return std::ptr::null_mut();
                        }
                        125 => {
                            state.borrow_mut().select_next_session();
                            return std::ptr::null_mut();
                        }
                        _ => {
                            return std::ptr::null_mut();
                        }
                    }
                }

                if in_focus && key_code == 53 {
                    state.borrow_mut().workspace.return_to_battlefield();
                    window.makeFirstResponder(None);
                    return std::ptr::null_mut();
                }

                event.as_ptr()
            },
        );

        let monitor = unsafe {
            NSEvent::addLocalMonitorForEventsMatchingMask_handler(
                NSEventMask::KeyDown,
                &key_block,
            )
        };

        KeyMonitorHandle { monitor }
    }

    /// Returns whether the currently selected session is embedded (visible as a live
    /// terminal) in the battlefield view given the provided content frame.
    pub fn battlefield_embeds_selected_terminal(
        state: &AppState,
        content_frame: NSRect,
    ) -> bool {
        let Some(session_id) = state.workspace.selected_session() else {
            return false;
        };
        workspace_support::embedded_session_ids(
            &state.card_render_data(),
            content_frame,
            state.workspace.focused_session(),
        )
        .contains(&session_id)
    }
}

#[cfg(target_os = "macos")]
pub use imp::*;
