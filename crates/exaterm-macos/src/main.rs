#[cfg(target_os = "macos")]
mod app_delegate;
#[cfg(target_os = "macos")]
mod menu;
#[cfg(target_os = "macos")]
mod session_io;

#[cfg(target_os = "macos")]
use exaterm_macos::app_state;
#[cfg(target_os = "macos")]
use exaterm_macos::battlefield_view;
#[cfg(target_os = "macos")]
use exaterm_macos::empty_state_view;
#[cfg(target_os = "macos")]
use exaterm_macos::focus_view;
#[cfg(target_os = "macos")]
use exaterm_macos::style;
#[cfg(target_os = "macos")]
use exaterm_macos::terminal_view;
#[cfg(target_os = "macos")]
use exaterm_macos::window;
#[cfg(target_os = "macos")]
use exaterm_macos::key_monitor;
#[cfg(target_os = "macos")]
use exaterm_macos::workspace_support;

#[cfg(target_os = "macos")]
use std::cell::RefCell;
#[cfg(target_os = "macos")]
use std::collections::{BTreeMap, BTreeSet};
#[cfg(target_os = "macos")]
use std::rc::Rc;
#[cfg(target_os = "macos")]
use std::sync::atomic::AtomicBool;
#[cfg(target_os = "macos")]
use std::sync::Arc;

#[cfg(target_os = "macos")]
use objc2_foundation::{NSPoint, NSRect, NSSize};

#[cfg(target_os = "macos")]
fn main() {
    let argv = std::env::args().collect::<Vec<_>>();
    if argv.get(1).map(|s| s.as_str()) == Some("--beachhead-daemon") {
        let code = exaterm_core::run_local_daemon();
        std::process::exit(if code == std::process::ExitCode::SUCCESS {
            0
        } else {
            1
        });
    }
    let mode = match exaterm_ui::beachhead::parse_run_mode(argv.into_iter().skip(1)) {
        Ok(mode) => mode,
        Err(error) => {
            eprintln!("{error}");
            eprintln!("usage: exaterm [--ssh user@host]");
            std::process::exit(2);
        }
    };
    run_app(mode);
}

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("exaterm-macos is only supported on macOS");
}

#[cfg(target_os = "macos")]
fn run_app(mode: exaterm_ui::beachhead::RunMode) {
    use objc2::msg_send;
    use objc2::rc::Retained;
    use objc2::runtime::ProtocolObject;
    use objc2::{MainThreadMarker, MainThreadOnly};
    use objc2_app_kit::{
        NSApplication, NSApplicationActivationPolicy, NSBackingStoreType, NSView, NSWindow,
        NSWindowStyleMask,
    };
    use objc2_foundation::{ns_string, NSPoint, NSRect, NSSize};

    let mtm = MainThreadMarker::new().expect("must be called from the main thread");

    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Regular);

    // Create the delegate.
    let delegate = app_delegate::AppDelegate::new(mtm);
    app.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));

    // Connect to the daemon.
    let target = exaterm_ui::beachhead::BeachheadTarget::from(&mode);
    let beachhead = match exaterm_ui::beachhead::BeachheadConnection::connect(&target) {
        Ok(client) => client,
        Err(error) => {
            present_startup_error(mtm, &error);
            std::process::exit(1);
        }
    };

    // Request the default workspace.
    let _ = beachhead
        .commands()
        .send(exaterm_types::proto::ClientMessage::CreateOrResumeDefaultWorkspace);

    // Store command sender for menu actions (thread-local, used by AppDelegate).
    app_delegate::set_command_sender(beachhead.commands().clone());
    // Wire the same sender into the battlefield view for budget dispatch.
    battlefield_view::set_budget_sender(beachhead.commands().clone());
    focus_view::set_focus_budget_sender(beachhead.commands().clone());

    let beachhead = Rc::new(beachhead);

    // Shared mutable state.
    let state = Rc::new(RefCell::new(app_state::AppState::new()));
    let session_ios = Rc::new(RefCell::new(session_io::SessionIOMap::new()));
    let sync_inputs_enabled = Arc::new(AtomicBool::new(false));
    app_delegate::set_sync_inputs_state(sync_inputs_enabled.clone());
    let terminal_surfaces = Rc::new(RefCell::new(BTreeMap::<
        exaterm_types::model::SessionId,
        TerminalSurface,
    >::new()));
    let key_surfaces = Rc::new(RefCell::new(BTreeMap::<
        exaterm_types::model::SessionId,
        objc2::rc::Retained<objc2_app_kit::NSView>,
    >::new()));

    // Create and configure the main window.
    let style_mask = NSWindowStyleMask::Titled
        | NSWindowStyleMask::Closable
        | NSWindowStyleMask::Miniaturizable
        | NSWindowStyleMask::Resizable;

    let content_rect = NSRect::new(
        NSPoint::new(200.0, 200.0),
        NSSize::new(window::WINDOW_DEFAULT_WIDTH, window::WINDOW_DEFAULT_HEIGHT),
    );

    let main_window: Retained<NSWindow> = unsafe {
        NSWindow::initWithContentRect_styleMask_backing_defer(
            NSWindow::alloc(mtm),
            content_rect,
            style_mask,
            NSBackingStoreType::Buffered,
            false,
        )
    };

    main_window.setTitle(ns_string!("Exaterm"));
    main_window.setMinSize(NSSize::new(
        window::WINDOW_MIN_WIDTH,
        window::WINDOW_MIN_HEIGHT,
    ));

    // Dark appearance.
    use objc2_app_kit::{NSAppearance, NSAppearanceCustomization, NSAppearanceName};
    let dark_name: &NSAppearanceName = unsafe { objc2_app_kit::NSAppearanceNameDarkAqua };
    if let Some(dark) = NSAppearance::appearanceNamed(dark_name) {
        main_window.setAppearance(Some(&dark));
    }

    // Window background from theme.
    let bg = style::color_to_nscolor(&window::window_background());
    main_window.setBackgroundColor(Some(&bg));

    let content_view = NSView::initWithFrame(
        NSView::alloc(mtm),
        NSRect::new(
            NSPoint::new(0.0, 0.0),
            NSSize::new(window::WINDOW_DEFAULT_WIDTH, window::WINDOW_DEFAULT_HEIGHT),
        ),
    );
    let empty_state = empty_state_view::build_empty_state(mtm, content_view.frame());

    let battlefield_view: Retained<battlefield_view::BattlefieldView> = unsafe {
        let frame = NSRect::new(
            NSPoint::new(0.0, 0.0),
            NSSize::new(window::WINDOW_DEFAULT_WIDTH, window::WINDOW_DEFAULT_HEIGHT),
        );
        msg_send![battlefield_view::BattlefieldView::alloc(mtm), initWithFrame: frame]
    };
    let focus_panel: Retained<focus_view::FocusView> = unsafe {
        let frame = NSRect::new(
            NSPoint::new(0.0, 0.0),
            NSSize::new(
                window::WINDOW_DEFAULT_WIDTH,
                window::WINDOW_DEFAULT_HEIGHT - 240.0,
            ),
        );
        msg_send![focus_view::FocusView::alloc(mtm), initWithFrame: frame]
    };

    empty_state.container.setHidden(true);
    battlefield_view.setHidden(false);
    focus_panel.setHidden(true);

    // Use autoresizing masks so both views fill the content view.
    battlefield_view.setAutoresizingMask(
        objc2_app_kit::NSAutoresizingMaskOptions::ViewWidthSizable
            | objc2_app_kit::NSAutoresizingMaskOptions::ViewHeightSizable,
    );
    battlefield_view.setFrame(content_view.frame());
    focus_panel.setAutoresizingMask(
        objc2_app_kit::NSAutoresizingMaskOptions::ViewWidthSizable
            | objc2_app_kit::NSAutoresizingMaskOptions::ViewHeightSizable,
    );

    let battlefield_state = Rc::clone(&state);
    let battlefield_window = main_window.clone();
    let interaction_window = battlefield_window.clone();
    let interaction_surfaces = Rc::clone(&terminal_surfaces);
    battlefield_view::set_interaction_handler(move |interaction| match interaction {
        battlefield_view::BattlefieldInteraction::Select(session_id) => {
            let frame = interaction_window
                .contentView()
                .map(|view| view.frame())
                .unwrap_or_else(|| {
                    NSRect::new(
                        NSPoint::new(0.0, 0.0),
                        NSSize::new(window::WINDOW_DEFAULT_WIDTH, window::WINDOW_DEFAULT_HEIGHT),
                    )
                });
            match workspace_support::activate_battlefield_session(
                &mut battlefield_state.borrow_mut(),
                frame,
                session_id,
            ) {
                workspace_support::BattlefieldActivation::SelectedEmbedded(session_id) => {
                    if let Some(surface) = interaction_surfaces.borrow().get(&session_id) {
                        interaction_window.makeFirstResponder(Some(&*surface.view));
                    } else {
                        interaction_window.makeFirstResponder(None);
                    }
                }
                workspace_support::BattlefieldActivation::Focused(_)
                | workspace_support::BattlefieldActivation::ReturnedToBattlefield => {
                    interaction_window.makeFirstResponder(None);
                }
            }
        }
        battlefield_view::BattlefieldInteraction::Focus(session_id) => {
            battlefield_state
                .borrow_mut()
                .workspace
                .enter_focus_mode(session_id);
            interaction_window.makeFirstResponder(None);
        }
    });

    content_view.addSubview(&empty_state.container);
    content_view.addSubview(&battlefield_view);
    content_view.addSubview(&focus_panel);
    main_window.setContentView(Some(&content_view));

    // Build and set the menu bar.
    let menu_bar = menu::build_menu_bar(mtm);
    app.setMainMenu(Some(&menu_bar));

    // Set up a 100ms repeating timer to drain daemon events, session output, and refresh display.
    let timer_state = Rc::clone(&state);
    let timer_empty_state = empty_state.container.clone();
    let timer_battlefield_view = battlefield_view.clone();
    let timer_focus_panel = focus_panel.clone();
    let timer_ios = Rc::clone(&session_ios);
    let render_state = Rc::new(terminal_view::TerminalRenderState::new());

    let stream_processors = Rc::new(RefCell::new(BTreeMap::<
        exaterm_types::model::SessionId,
        exaterm_core::terminal_stream::TerminalStreamProcessor,
    >::new()));

    let headless_terminals = Rc::new(RefCell::new(BTreeMap::<
        exaterm_types::model::SessionId,
        exaterm_core::headless_terminal::HeadlessTerminal,
    >::new()));

    let timer_beachhead = Rc::clone(&beachhead);
    let displayed_focus = Rc::new(RefCell::new(None::<exaterm_types::model::SessionId>));

    let timer_displayed_focus = Rc::clone(&displayed_focus);
    let timer_surfaces = Rc::clone(&terminal_surfaces);
    let timer_key_surfaces = Rc::clone(&key_surfaces);
    let timer_processors = Rc::clone(&stream_processors);
    let timer_headless = Rc::clone(&headless_terminals);
    let timer_sync_inputs = sync_inputs_enabled.clone();
    let timer_block = block2::StackBlock::new(
        move |_timer: std::ptr::NonNull<objc2_foundation::NSTimer>| {
            // Drain all pending events from the daemon.
            while let Ok(message) = timer_beachhead.events().try_recv() {
                match message {
                    exaterm_types::proto::ServerMessage::WorkspaceSnapshot { snapshot } => {
                        timer_state.borrow_mut().apply_snapshot(&snapshot);
                    }
                    exaterm_types::proto::ServerMessage::Error { message } => {
                        eprintln!("exaterm: daemon error: {message}");
                    }
                }
            }
            timer_beachhead.drain_event_wake();

            // Update the first session ID for menu actions (e.g., New Shell).
            let borrowed_state = timer_state.borrow();
            let first_id = borrowed_state.workspace.sessions().first().map(|s| s.id);
            app_delegate::set_first_session(first_id);
            app_delegate::set_selected_session(borrowed_state.workspace.selected_session());
            app_delegate::set_has_sessions(!borrowed_state.workspace.sessions().is_empty());
            let selected_auto_nudge = borrowed_state
                .workspace
                .selected_session()
                .and_then(|id| borrowed_state.auto_nudge_enabled.get(&id).copied())
                .unwrap_or(false);
            app_delegate::set_selected_auto_nudge(selected_auto_nudge);
            drop(borrowed_state);

            // Connect to any new session raw streams.
            {
                let borrowed = timer_state.borrow();
                let mut ios = timer_ios.borrow_mut();
                ios.connect_new_sessions(
                    &timer_beachhead.raw_session_connector(),
                    &borrowed.raw_socket_names,
                );

                // Remove sessions that are no longer present.
                let active_ids: Vec<_> = borrowed.raw_socket_names.keys().copied().collect();
                ios.retain_sessions(&active_ids);
            }

            // Drain all PTY output every tick to prevent background buffer growth.
            let all_output = timer_ios.borrow_mut().drain_all_output();
            {
                let mut stream_updates = Vec::new();
                let mut rendered_updates: Vec<(exaterm_types::model::SessionId, Vec<String>)> =
                    Vec::new();
                for (session_id, bytes) in &all_output {
                    if let Some(surface) = timer_surfaces.borrow().get(session_id) {
                        surface.bridge.feed(bytes);
                    }
                    let update = timer_processors
                        .borrow_mut()
                        .entry(*session_id)
                        .or_default()
                        .ingest(bytes);
                    if !update.semantic_lines.is_empty() {
                        stream_updates.push((*session_id, update.semantic_lines));
                    }
                    timer_headless
                        .borrow_mut()
                        .entry(*session_id)
                        .or_default()
                        .ingest(bytes);
                    let rendered = timer_headless
                        .borrow()
                        .get(session_id)
                        .map(|h| h.rendered_lines(24))
                        .unwrap_or_default();
                    if !rendered.is_empty() {
                        rendered_updates.push((*session_id, rendered));
                    }
                }
                if !stream_updates.is_empty() || !rendered_updates.is_empty() {
                    let mut state = timer_state.borrow_mut();
                    for (session_id, lines) in stream_updates {
                        exaterm_core::observation::append_recent_lines(
                            state
                                .recent_lines
                                .entry(session_id)
                                .or_insert_with(Vec::new),
                            &lines,
                        );
                    }
                    for (session_id, rendered) in rendered_updates {
                        state.rendered_scrollback.insert(session_id, rendered);
                    }
                }
            }

            let content_bounds = content_view.frame();
            let borrowed = timer_state.borrow();
            ensure_terminal_surfaces(
                &mut timer_surfaces.borrow_mut(),
                borrowed.workspace.sessions(),
                &timer_ios,
                &timer_beachhead,
                timer_sync_inputs.clone(),
            );

            // Sync views-only map for the key monitor.
            {
                let surfs = timer_surfaces.borrow();
                let mut key_surfs = timer_key_surfaces.borrow_mut();
                key_surfs.retain(|id, _| surfs.contains_key(id));
                for (id, surface) in surfs.iter() {
                    key_surfs.entry(*id).or_insert_with(|| surface.view.clone());
                }
            }

            let focused = borrowed.workspace.focused_session();
            {
                let mut displayed = timer_displayed_focus.borrow_mut();
                if *displayed != focused {
                    *displayed = focused;
                }
            }

            let cards = borrowed.card_render_data();
            let selected = borrowed.workspace.selected_session();
            let card_rects = exaterm_ui::layout::card_layout(
                cards.len(),
                content_bounds.size.width,
                if focused.is_some() {
                    workspace_support::FOCUS_RAIL_HEIGHT
                } else {
                    content_bounds.size.height
                },
            );
            let embedded_ids =
                workspace_support::embedded_session_ids(&cards, content_bounds, focused);
            layout_views(
                &content_view,
                &timer_empty_state,
                &timer_battlefield_view,
                &timer_focus_panel,
                !cards.is_empty(),
                focused,
            );
            battlefield_view::set_battlefield_data(
                cards.clone(),
                selected,
                Rc::clone(&render_state),
                embedded_ids.clone(),
                focused.is_some(),
            );
            focus_view::set_focus_data(
                focused.and_then(|session_id| borrowed.focus_render_data(session_id)),
                Rc::clone(&render_state),
            );
            timer_battlefield_view.setNeedsDisplay(true);
            timer_focus_panel.setNeedsDisplay(true);
            apply_terminal_layout(
                &timer_surfaces.borrow(),
                &timer_battlefield_view,
                &timer_focus_panel,
                &cards,
                &card_rects,
                &embedded_ids,
                focused,
            );
            if let Some(session_id) = focused {
                if let Some(surface) = timer_surfaces.borrow().get(&session_id) {
                    battlefield_window.makeFirstResponder(Some(&*surface.view));
                }
            } else if let Some(selected_id) = selected {
                // In embedded battlefield mode the selected terminal should receive
                // keyboard input directly. Set it as first responder so the user
                // doesn't need an initial click — mirrors what BattlefieldInteraction
                // does on mouse click, but runs on startup before any click occurs.
                if embedded_ids.contains(&selected_id) {
                    if let Some(surface) = timer_surfaces.borrow().get(&selected_id) {
                        battlefield_window.makeFirstResponder(Some(&*surface.view));
                    }
                }
            }
        },
    );

    // SAFETY: Block captures only main-thread state and timer fires on the main run loop.
    let _timer = unsafe {
        objc2_foundation::NSTimer::scheduledTimerWithTimeInterval_repeats_block(
            0.1,
            true,
            &timer_block,
        )
    };

    // Set up keyboard event monitoring via the shared library function.
    let _key_monitor = key_monitor::register_battlefield_key_monitor(
        Rc::clone(&state),
        main_window.clone(),
        Rc::clone(&key_surfaces),
        app_delegate::send_add_terminals,
    );

    // Defer window show and activation into applicationDidFinishLaunching so they are
    // delivered while the run loop is processing events. Calling these before app.run()
    // can leave the activation undelivered, meaning the window appears but keyboard focus
    // stays in the launching terminal until the user clicks.
    let launch_window = main_window.clone();
    app_delegate::set_launch_handler(move || {
        let mtm = MainThreadMarker::new().expect("main thread");
        // activateIgnoringOtherApps(true) forces the app to the front regardless of
        // which app is currently active. The newer activate() API explicitly provides
        // "no guarantee that the app will be activated at all" — it requires the
        // current frontmost app to cooperate, which a terminal launching us will not.
        #[allow(deprecated)]
        NSApplication::sharedApplication(mtm).activateIgnoringOtherApps(true);
        launch_window.makeKeyAndOrderFront(None);
    });

    // Keep everything alive for the lifetime of the app.
    std::mem::forget(main_window);
    std::mem::forget(beachhead);
    std::mem::forget(state);
    std::mem::forget(session_ios);
    std::mem::forget(sync_inputs_enabled);
    std::mem::forget(terminal_surfaces);
    std::mem::forget(key_surfaces);
    std::mem::forget(_key_monitor);

    app.run();
}

#[cfg(target_os = "macos")]
struct TerminalSurface {
    bridge: Rc<exaterm_swiftterm::TerminalBridge>,
    view: objc2::rc::Retained<objc2_app_kit::NSView>,
}

#[cfg(target_os = "macos")]
#[derive(Clone, Copy, Debug, PartialEq)]
enum TerminalPlacement {
    Hidden,
    Focused(NSRect),
    Embedded(NSRect),
}

#[cfg(target_os = "macos")]
fn terminal_appearance() -> exaterm_swiftterm::TerminalAppearance {
    let terminal_font = exaterm_ui::theme::terminal_font();
    exaterm_swiftterm::TerminalAppearance {
        font_name: style::font_family(&terminal_font).to_string(),
        font_size: terminal_font.size as f64,
        foreground: exaterm_ui::theme::terminal_foreground_color(),
        background: exaterm_ui::theme::terminal_background_color(),
        cursor: exaterm_ui::theme::terminal_cursor_color(),
    }
}

#[cfg(target_os = "macos")]
fn ensure_terminal_surfaces(
    surfaces: &mut BTreeMap<exaterm_types::model::SessionId, TerminalSurface>,
    sessions: &[exaterm_types::model::SessionRecord],
    ios: &Rc<RefCell<session_io::SessionIOMap>>,
    beachhead: &Rc<exaterm_ui::beachhead::BeachheadConnection>,
    sync_inputs_enabled: Arc<AtomicBool>,
) {
    let active_ids: BTreeSet<_> = sessions.iter().map(|session| session.id).collect();
    surfaces.retain(|id, _| active_ids.contains(id));
    for session in sessions {
        surfaces.entry(session.id).or_insert_with(|| {
            let bridge = Rc::new(exaterm_swiftterm::TerminalBridge::new(NSRect::new(
                NSPoint::new(0.0, 0.0),
                NSSize::new(640.0, 360.0),
            )));
            bridge.set_appearance(&terminal_appearance());
            let session_id = session.id;
            let ios = Rc::clone(ios);
            let sync = sync_inputs_enabled.clone();
            bridge.set_input_handler(move |bytes: &[u8]| {
                if sync.load(std::sync::atomic::Ordering::Relaxed) {
                    ios.borrow_mut().write_input_all(bytes);
                } else {
                    ios.borrow_mut().write_input(&session_id, bytes);
                }
            });
            let commands = beachhead.commands().clone();
            bridge.set_size_handler(move |size| {
                let _ = commands.send(exaterm_types::proto::ClientMessage::ResizeTerminal {
                    session_id,
                    rows: size.rows,
                    cols: size.cols,
                });
            });
            let view = bridge.view();
            view.setAutoresizingMask(
                objc2_app_kit::NSAutoresizingMaskOptions::ViewWidthSizable
                    | objc2_app_kit::NSAutoresizingMaskOptions::ViewHeightSizable,
            );
            TerminalSurface { bridge, view }
        });
    }
}

#[cfg(target_os = "macos")]
fn layout_views(
    content_view: &objc2_app_kit::NSView,
    empty_state: &objc2_app_kit::NSView,
    battlefield_view: &battlefield_view::BattlefieldView,
    focus_panel: &focus_view::FocusView,
    has_sessions: bool,
    focused: Option<exaterm_types::model::SessionId>,
) {
    let frame = content_view.frame();
    let layout = workspace_support::workspace_layout(frame, has_sessions, focused);

    empty_state.setHidden(!layout.empty_state_visible);
    battlefield_view.setHidden(!layout.battlefield_visible);
    focus_panel.setHidden(!layout.focus_visible);
    battlefield_view.setFrame(layout.battlefield_frame);
    if layout.focus_visible {
        focus_panel.setFrame(layout.focus_frame);
    }
}

#[cfg(target_os = "macos")]
fn apply_terminal_layout(
    surfaces: &BTreeMap<exaterm_types::model::SessionId, TerminalSurface>,
    battlefield_view: &battlefield_view::BattlefieldView,
    focus_panel: &focus_view::FocusView,
    cards: &[app_state::CardRenderData],
    rects: &[exaterm_ui::layout::CardRect],
    embedded_ids: &BTreeSet<exaterm_types::model::SessionId>,
    focused: Option<exaterm_types::model::SessionId>,
) {
    let focus_panel_size = focus_panel.frame().size;
    for (session_id, surface) in surfaces {
        let placement = terminal_placement_for_session(
            *session_id,
            focus_panel_size.width as i32,
            focus_panel_size.height as i32,
            cards,
            rects,
            embedded_ids,
            focused,
        );
        apply_terminal_surface_placement(surface, battlefield_view, focus_panel, placement);
    }
}

#[cfg(target_os = "macos")]
fn terminal_placement_for_session(
    session_id: exaterm_types::model::SessionId,
    focus_panel_width: i32,
    focus_panel_height: i32,
    cards: &[app_state::CardRenderData],
    rects: &[exaterm_ui::layout::CardRect],
    embedded_ids: &BTreeSet<exaterm_types::model::SessionId>,
    focused: Option<exaterm_types::model::SessionId>,
) -> TerminalPlacement {
    if focused == Some(session_id) {
        let slot =
            exaterm_ui::layout::focus_terminal_slot_rect(focus_panel_width, focus_panel_height);
        return TerminalPlacement::Focused(NSRect::new(
            NSPoint::new(slot.x, slot.y),
            NSSize::new(slot.w, slot.h),
        ));
    }

    if embedded_ids.contains(&session_id) {
        if let Some((_, rect)) = cards
            .iter()
            .zip(rects.iter())
            .find(|(card, _)| card.id == session_id)
        {
            let slot = exaterm_ui::layout::card_terminal_slot_rect(rect);
            return TerminalPlacement::Embedded(NSRect::new(
                NSPoint::new(slot.x, slot.y),
                NSSize::new(slot.w, slot.h),
            ));
        }
    }

    TerminalPlacement::Hidden
}

#[cfg(target_os = "macos")]
fn apply_terminal_surface_placement(
    surface: &TerminalSurface,
    battlefield_view: &battlefield_view::BattlefieldView,
    focus_panel: &focus_view::FocusView,
    placement: TerminalPlacement,
) {
    match placement {
        TerminalPlacement::Hidden => {
            if unsafe { surface.view.superview() }.is_some() {
                surface.view.removeFromSuperview();
            }
            if !surface.view.isHidden() {
                surface.view.setHidden(true);
            }
        }
        TerminalPlacement::Focused(frame) => {
            ensure_view_parent(&surface.view, focus_panel);
            update_view_frame(&surface.view, frame);
            if surface.view.isHidden() {
                surface.view.setHidden(false);
            }
        }
        TerminalPlacement::Embedded(frame) => {
            ensure_view_parent(&surface.view, battlefield_view);
            update_view_frame(&surface.view, frame);
            if surface.view.isHidden() {
                surface.view.setHidden(false);
            }
        }
    }
}

#[cfg(target_os = "macos")]
fn ensure_view_parent(view: &objc2_app_kit::NSView, target: &objc2_app_kit::NSView) {
    if unsafe { view.superview() }
        .as_deref()
        .is_some_and(|parent| std::ptr::eq(parent, target))
    {
        return;
    }

    if unsafe { view.superview() }.is_some() {
        view.removeFromSuperview();
    }
    target.addSubview(view);
}

#[cfg(target_os = "macos")]
fn update_view_frame(view: &objc2_app_kit::NSView, frame: NSRect) {
    if view.frame() != frame {
        view.setFrame(frame);
    }
}

#[cfg(target_os = "macos")]
fn present_startup_error(mtm: objc2::MainThreadMarker, error: &str) {
    use objc2_app_kit::{NSAlert, NSAlertStyle};
    use objc2_foundation::NSString;

    let alert = NSAlert::new(mtm);
    alert.setAlertStyle(NSAlertStyle::Critical);
    let message = NSString::from_str("Exaterm could not start a live beachhead connection.");
    let info = NSString::from_str(error);
    alert.setMessageText(&message);
    alert.setInformativeText(&info);
    alert.runModal();
}

#[cfg(test)]
#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::terminal_placement_for_session;
    use super::TerminalPlacement;
    use exaterm_macos::app_state::CardRenderData;
    use exaterm_types::model::SessionId;
    use exaterm_ui::layout::{card_terminal_slot_rect, CardRect};
    use exaterm_ui::presentation::nudge_state_presentation;
    use exaterm_ui::supervision::BattleCardStatus;
    use objc2_foundation::{NSPoint, NSRect, NSSize};
    use std::collections::BTreeSet;

    fn test_card(id: SessionId) -> CardRenderData {
        CardRenderData {
            id,
            title: "session".into(),
            status: BattleCardStatus::Active,
            status_label: "ACTIVE".into(),
            recency: "now".into(),
            scrollback: Vec::new(),
            headline: String::new(),
            combined_headline: String::new(),
            detail: None,
            alert: None,
            attention: None,
            attention_reason: None,
            attention_bar: None,
            attention_bar_reason: None,
            nudge_state: nudge_state_presentation(false, false, false),
            last_nudge: None,
        }
    }

    #[test]
    fn terminal_placement_equality_is_variant_sensitive() {
        let frame = NSRect::new(NSPoint::new(1.0, 2.0), NSSize::new(3.0, 4.0));
        assert_eq!(TerminalPlacement::Hidden, TerminalPlacement::Hidden);
        assert_eq!(
            TerminalPlacement::Focused(frame),
            TerminalPlacement::Focused(frame)
        );
        assert_eq!(
            TerminalPlacement::Embedded(frame),
            TerminalPlacement::Embedded(frame)
        );
        assert_ne!(TerminalPlacement::Hidden, TerminalPlacement::Focused(frame));
        assert_ne!(
            TerminalPlacement::Focused(frame),
            TerminalPlacement::Embedded(frame)
        );
    }

    #[test]
    fn placement_is_hidden_when_session_is_not_visible() {
        let cards = vec![test_card(SessionId(1))];
        let rects = vec![CardRect {
            x: 12.0,
            y: 12.0,
            w: 640.0,
            h: 480.0,
        }];

        let placement = terminal_placement_for_session(
            SessionId(2),
            1200,
            700,
            &cards,
            &rects,
            &BTreeSet::from([SessionId(1)]),
            None,
        );

        assert_eq!(placement, TerminalPlacement::Hidden);
    }

    #[test]
    fn placement_uses_focus_slot_for_focused_session() {
        let placement = terminal_placement_for_session(
            SessionId(1),
            1200,
            700,
            &[],
            &[],
            &BTreeSet::new(),
            Some(SessionId(1)),
        );

        match placement {
            TerminalPlacement::Focused(frame) => {
                assert!(frame.size.width > 0.0);
                assert!(frame.size.height > 0.0);
            }
            other => panic!("expected focused placement, got {other:?}"),
        }
    }

    #[test]
    fn placement_uses_card_slot_for_embedded_session() {
        let card = CardRect {
            x: 12.0,
            y: 12.0,
            w: 700.0,
            h: 520.0,
        };
        let expected = card_terminal_slot_rect(&card);

        let placement = terminal_placement_for_session(
            SessionId(1),
            1200,
            700,
            &[test_card(SessionId(1))],
            &[card],
            &BTreeSet::from([SessionId(1)]),
            None,
        );

        assert_eq!(
            placement,
            TerminalPlacement::Embedded(NSRect::new(
                NSPoint::new(expected.x, expected.y),
                NSSize::new(expected.w, expected.h),
            ))
        );
    }

    #[test]
    fn placement_is_hidden_when_embedded_session_has_no_card_rect() {
        let placement = terminal_placement_for_session(
            SessionId(1),
            1200,
            700,
            &[test_card(SessionId(1))],
            &[],
            &BTreeSet::from([SessionId(1)]),
            None,
        );

        assert_eq!(placement, TerminalPlacement::Hidden);
    }
}
