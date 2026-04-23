#[cfg(target_os = "macos")]
mod imp {
    use std::cell::RefCell;
    use std::collections::BTreeMap;
    use std::rc::Rc;

    use exaterm_core::headless_terminal::HeadlessTerminal;
    use exaterm_core::observation::append_recent_lines;
    use exaterm_core::terminal_stream::TerminalStreamProcessor;
    use exaterm_types::model::SessionId;
    use exaterm_ui::app_state::{AppState, CardRenderData, FocusRenderData};
    use exaterm_ui::ui_test_contract::{scenario_fixture, selectors, UiSessionKey, UiTestScenario};
    use glasscheck::{
        Harness, NodeRecipe, Point, PropertyValue, Rect, RegionSpec, Role, SceneSource,
        SemanticNode, Size, WindowHost,
    };
    use objc2::rc::Retained;
    use objc2::{msg_send, MainThreadOnly};
    use objc2_app_kit::NSView;
    use objc2_foundation::{NSPoint, NSRect, NSSize};

    use crate::battlefield_view::{self, BattlefieldView};
    use crate::empty_state_view::{self, EmptyStateViews};
    use crate::focus_view::{self, FocusView};
    use crate::terminal_view::TerminalRenderState;
    use crate::workspace_support;

    #[derive(Debug)]
    pub struct MountError {
        message: String,
    }

    impl std::fmt::Display for MountError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", self.message)
        }
    }

    impl std::error::Error for MountError {}

    pub struct MountedAppKitUi {
        pub host: WindowHost,
        state: Rc<RefCell<AppState>>,
        battlefield_view: Retained<crate::battlefield_view::BattlefieldView>,
        render_state: Rc<TerminalRenderState>,
        stream_processors: RefCell<BTreeMap<SessionId, TerminalStreamProcessor>>,
        headless_terminals: RefCell<BTreeMap<SessionId, HeadlessTerminal>>,
    }

    impl MountedAppKitUi {
        /// Feed raw PTY bytes for a session through the stream processor and headless terminal.
        /// Updates `recent_lines` (and, once §4 lands, `rendered_scrollback`) in the state,
        /// then triggers a battlefield repaint.
        pub fn feed_session(&self, session_id: SessionId, bytes: &[u8]) {
            let update = self
                .stream_processors
                .borrow_mut()
                .entry(session_id)
                .or_default()
                .ingest(bytes);
            let rendered = {
                let mut map = self.headless_terminals.borrow_mut();
                let terminal = map.entry(session_id).or_default();
                terminal.ingest(bytes);
                terminal.rendered_lines(24)
            };

            let mut state = self.state.borrow_mut();
            append_recent_lines(
                state.recent_lines.entry(session_id).or_default(),
                &update.semantic_lines,
            );
            if !rendered.is_empty() {
                state.rendered_scrollback.insert(session_id, rendered);
            }
            let cards = state.card_render_data();
            let selected = state.workspace.selected_session();
            let focused = state.workspace.focused_session();
            drop(state);

            let embedded_ids = crate::workspace_support::embedded_session_ids(
                &cards,
                self.battlefield_view.frame(),
                focused,
            );
            battlefield_view::set_battlefield_data(
                cards,
                selected,
                Rc::clone(&self.render_state),
                embedded_ids,
                focused.is_some(),
            );
            self.battlefield_view.setNeedsDisplay(true);
        }
    }

    /// A mounted UI fixture that includes a live TerminalBridge wired to a key monitor.
    /// Used to test keyboard dispatch through the AppKit local event monitor.
    pub struct MountedAppKitUiWithTerminal {
        pub host: WindowHost,
        /// Bytes delivered to the terminal's input handler (i.e. bytes sent toward the PTY).
        pub received_bytes: Rc<RefCell<Vec<u8>>>,
        _key_monitor: crate::key_monitor::KeyMonitorHandle,
        _bridge: Rc<exaterm_swiftterm::TerminalBridge>,
    }

    /// Mounts a single-session embedded battlefield scenario with a real TerminalBridge
    /// and registers the AppKit keyboard event monitor.
    ///
    /// The returned `received_bytes` accumulates every byte that SwiftTerm's input
    /// handler delivers (i.e. keystrokes forwarded toward a PTY). Use it in tests to
    /// assert that a key event was not consumed by the event monitor.
    pub fn mount_with_terminal(
        harness: &Harness,
    ) -> Result<MountedAppKitUiWithTerminal, MountError> {
        use objc2_app_kit::NSWindow;

        let fixture = scenario_fixture(UiTestScenario::BattlefieldSingleSparse);
        let host =
            harness.create_window(fixture.window_width as f64, fixture.window_height as f64);
        let content_frame = NSRect::new(
            NSPoint::new(0.0, 0.0),
            NSSize::new(fixture.window_width as f64, fixture.window_height as f64),
        );
        let content_view = NSView::initWithFrame(
            NSView::alloc(harness.main_thread_marker()),
            content_frame,
        );

        let state = Rc::new(RefCell::new(AppState::new()));
        state.borrow_mut().apply_snapshot(&fixture.snapshot);
        if let Some(session_id) = fixture.selected_session {
            state.borrow_mut().workspace.select_session(session_id);
        }

        let bridge = Rc::new(exaterm_swiftterm::TerminalBridge::new(content_frame));
        let received = Rc::new(RefCell::new(Vec::<u8>::new()));
        let received_clone = Rc::clone(&received);
        bridge.set_input_handler(move |bytes: &[u8]| {
            received_clone.borrow_mut().extend_from_slice(bytes);
        });

        let bridge_view = bridge.view();
        bridge_view.setFrame(content_frame);
        content_view.addSubview(&bridge_view);
        host.set_content_view(&content_view);

        let session_id = UiSessionKey::Shell1.session_id();
        let mut surfaces_map = BTreeMap::new();
        surfaces_map.insert(session_id, bridge_view.clone());
        let surfaces = Rc::new(RefCell::new(surfaces_map));

        let window_retained: Retained<NSWindow> = unsafe {
            Retained::retain(host.window() as *const NSWindow as *mut NSWindow)
        }
        .ok_or_else(|| MountError {
            message: "window retain failed".into(),
        })?;

        let key_monitor = crate::key_monitor::register_battlefield_key_monitor(
            Rc::clone(&state),
            window_retained,
            surfaces,
            || {},
        );

        harness.settle(4);

        Ok(MountedAppKitUiWithTerminal {
            host,
            received_bytes: received,
            _key_monitor: key_monitor,
            _bridge: bridge,
        })
    }

    pub fn mount_scenario(
        harness: &Harness,
        scenario: UiTestScenario,
    ) -> Result<MountedAppKitUi, MountError> {
        let fixture = scenario_fixture(scenario);
        let host = harness.create_window(fixture.window_width as f64, fixture.window_height as f64);
        let content_frame = NSRect::new(
            NSPoint::new(0.0, 0.0),
            NSSize::new(fixture.window_width as f64, fixture.window_height as f64),
        );
        let content_view =
            NSView::initWithFrame(NSView::alloc(harness.main_thread_marker()), content_frame);
        let empty_state =
            empty_state_view::build_empty_state(harness.main_thread_marker(), content_frame);
        let battlefield_view: Retained<BattlefieldView> = unsafe {
            msg_send![BattlefieldView::alloc(harness.main_thread_marker()), initWithFrame: content_frame]
        };
        let focus_view: Retained<FocusView> = unsafe {
            msg_send![FocusView::alloc(harness.main_thread_marker()), initWithFrame: content_frame]
        };
        let render_state = Rc::new(TerminalRenderState::new());
        let state = Rc::new(RefCell::new(AppState::new()));
        state.borrow_mut().apply_snapshot(&fixture.snapshot);
        if let Some(session_id) = fixture.selected_session {
            state.borrow_mut().workspace.select_session(session_id);
        }

        let interaction_state = Rc::clone(&state);
        let interaction_root = content_view.clone();
        let interaction_empty_state = empty_state.container.clone();
        let interaction_empty_title = empty_state.title.clone();
        let interaction_empty_body = empty_state.body.clone();
        let interaction_battlefield_view = battlefield_view.clone();
        let interaction_focus_view = focus_view.clone();
        let interaction_render_state = Rc::clone(&render_state);
        battlefield_view::set_interaction_handler(move |interaction| {
            let empty_state = EmptyStateViews {
                container: interaction_empty_state.clone(),
                title: interaction_empty_title.clone(),
                body: interaction_empty_body.clone(),
            };
            match interaction {
                battlefield_view::BattlefieldInteraction::Select(session_id) => {
                    workspace_support::activate_battlefield_session(
                        &mut interaction_state.borrow_mut(),
                        interaction_root.frame(),
                        session_id,
                    );
                }
                battlefield_view::BattlefieldInteraction::Focus(session_id) => {
                    interaction_state
                        .borrow_mut()
                        .workspace
                        .enter_focus_mode(session_id);
                }
            }
            let state = interaction_state.borrow();
            refresh_views(
                &interaction_root,
                &empty_state,
                &interaction_battlefield_view,
                &interaction_focus_view,
                &interaction_render_state,
                &state,
            );
        });

        content_view.addSubview(&empty_state.container);
        content_view.addSubview(&battlefield_view);
        content_view.addSubview(&focus_view);
        host.set_content_view(&content_view);

        refresh_views(
            &content_view,
            &empty_state,
            &battlefield_view,
            &focus_view,
            &render_state,
            &state.borrow(),
        );

        host.set_scene_source(Box::new(AppKitSceneSource {
            root: content_view,
            empty_state,
            battlefield_view: battlefield_view.clone(),
            focus_view: focus_view.clone(),
            state: Rc::clone(&state),
        }));
        harness.settle(4);

        Ok(MountedAppKitUi {
            host,
            state,
            battlefield_view,
            render_state,
            stream_processors: RefCell::new(BTreeMap::new()),
            headless_terminals: RefCell::new(BTreeMap::new()),
        })
    }

    fn refresh_views(
        content_view: &NSView,
        empty_state: &EmptyStateViews,
        battlefield_view: &BattlefieldView,
        focus_view: &FocusView,
        render_state: &Rc<TerminalRenderState>,
        state: &AppState,
    ) {
        let frame = content_view.frame();
        let cards = state.card_render_data();
        let focused = state.workspace.focused_session();
        let selected = state.workspace.selected_session();
        let embedded_ids = workspace_support::embedded_session_ids(&cards, frame, focused);
        let layout = workspace_support::workspace_layout(frame, !cards.is_empty(), focused);

        empty_state.container.setHidden(!layout.empty_state_visible);
        battlefield_view.setHidden(!layout.battlefield_visible);
        focus_view.setHidden(!layout.focus_visible);
        battlefield_view.setFrame(layout.battlefield_frame);
        if layout.focus_visible {
            focus_view.setFrame(layout.focus_frame);
        }

        battlefield_view::set_battlefield_data(
            cards,
            selected,
            Rc::clone(render_state),
            embedded_ids,
            focused.is_some(),
        );
        focus_view::set_focus_data(
            focused.and_then(|session_id| state.focus_render_data(session_id)),
            Rc::clone(render_state),
        );
        battlefield_view.setNeedsDisplay(true);
        focus_view.setNeedsDisplay(true);
    }

    struct AppKitSceneSource {
        root: Retained<NSView>,
        empty_state: EmptyStateViews,
        battlefield_view: Retained<BattlefieldView>,
        focus_view: Retained<FocusView>,
        state: Rc<RefCell<AppState>>,
    }

    impl SceneSource for AppKitSceneSource {
        fn snapshot_nodes(&self) -> Vec<SemanticNode> {
            let state = self.state.borrow();
            let cards = state.card_render_data();
            let focused = state.workspace.focused_session();
            let embedded_ids =
                workspace_support::embedded_session_ids(&cards, self.root.frame(), focused);
            let mut nodes = Vec::new();

            let layout =
                workspace_support::workspace_layout(self.root.frame(), !cards.is_empty(), focused);
            if layout.empty_state_visible {
                nodes.push(node(
                    "workspace-empty-state",
                    selectors::WORKSPACE_EMPTY_STATE,
                    Role::Placeholder,
                    rect_from_nsrect(self.empty_state.container.frame()),
                    None,
                ));
                nodes.push(node(
                    "workspace-empty-state-title",
                    selectors::WORKSPACE_EMPTY_STATE_TITLE,
                    Role::Label,
                    rect_from_nsrect(self.empty_state.title.frame()),
                    Some(empty_state_view::EMPTY_STATE_TITLE.to_string()),
                ));
                nodes.push(node(
                    "workspace-empty-state-body",
                    selectors::WORKSPACE_EMPTY_STATE_BODY,
                    Role::Label,
                    rect_from_nsrect(self.empty_state.body.frame()),
                    Some(empty_state_view::EMPTY_STATE_BODY.to_string()),
                ));
            }

            if layout.battlefield_visible {
                nodes.push(node(
                    "workspace-battlefield",
                    selectors::WORKSPACE_BATTLEFIELD,
                    Role::List,
                    rect_from_nsrect(self.battlefield_view.frame()),
                    None,
                ));
            }

            if layout.focus_visible {
                nodes.push(node(
                    "workspace-focus-panel",
                    selectors::WORKSPACE_FOCUS_PANEL,
                    Role::Container,
                    rect_from_nsrect(self.focus_view.frame()),
                    None,
                ));
            }

            if let Some(focused_session) = focused {
                if let Some(key) = session_key(focused_session) {
                    if let Some(data) = state.focus_render_data(focused_session) {
                        push_focus_nodes(&mut nodes, &self.focus_view.frame(), key, &data);
                    }
                }
            }

            for (card, rect) in cards.iter().zip(battlefield_view::layout_for_mode(
                cards.len(),
                self.battlefield_view.frame(),
                focused.is_some(),
            )) {
                let Some(key) = session_key(card.id) else {
                    continue;
                };
                let regions = battlefield_regions(
                    card,
                    &NSRect::new(NSPoint::new(rect.x, rect.y), NSSize::new(rect.w, rect.h)),
                    embedded_ids.contains(&card.id),
                    focused.is_some(),
                );
                if let Some(title) = regions.title {
                    nodes.push(node(
                        &format!("battlefield-card-title-{}", key.slug()),
                        &selectors::battlefield_card_title(key),
                        Role::Label,
                        rect_from_nsrect(rect_in_root_from_flipped_local(
                            self.battlefield_view.frame(),
                            title,
                        )),
                        Some(card.title.clone()),
                    ));
                }
                if let Some(status) = regions.status {
                    nodes.push(node(
                        &format!("battlefield-card-status-{}", key.slug()),
                        &selectors::battlefield_card_status(key),
                        Role::Label,
                        rect_from_nsrect(rect_in_root_from_flipped_local(
                            self.battlefield_view.frame(),
                            status,
                        )),
                        Some(card.status_label.clone()),
                    ));
                }
                if let Some(headline) = regions.headline {
                    nodes.push(node(
                        &format!("battlefield-card-headline-{}", key.slug()),
                        &selectors::battlefield_card_headline(key),
                        Role::Label,
                        rect_from_nsrect(rect_in_root_from_flipped_local(
                            self.battlefield_view.frame(),
                            headline,
                        )),
                        Some(card.headline.clone()),
                    ));
                }
                if let Some(subtitle) = regions.subtitle {
                    nodes.push(node(
                        &format!("battlefield-card-subtitle-{}", key.slug()),
                        &selectors::battlefield_card_subtitle(key),
                        Role::Label,
                        rect_from_nsrect(rect_in_root_from_flipped_local(
                            self.battlefield_view.frame(),
                            subtitle,
                        )),
                        Some(card.headline.clone()),
                    ));
                }
                if let Some(alert) = regions.alert {
                    if let Some(alert_text) = card.alert.as_ref() {
                        nodes.push(node(
                            &format!("battlefield-card-alert-{}", key.slug()),
                            &selectors::battlefield_card_alert(key),
                            Role::Label,
                            rect_from_nsrect(rect_in_root_from_flipped_local(
                                self.battlefield_view.frame(),
                                alert,
                            )),
                            Some(format!("! {alert_text}")),
                        ));
                    }
                }
                if let Some(nudge) = regions.nudge {
                    nodes.push(node(
                        &format!("battlefield-card-nudge-{}", key.slug()),
                        &selectors::battlefield_card_nudge(key),
                        Role::Label,
                        rect_from_nsrect(rect_in_root_from_flipped_local(
                            self.battlefield_view.frame(),
                            nudge,
                        )),
                        Some(card.nudge_state.label.to_string()),
                    ));
                }
                if let Some(attention_bar) = regions.attention_bar {
                    nodes.push(node(
                        &format!("battlefield-card-attention-bar-{}", key.slug()),
                        &selectors::battlefield_card_attention_bar(key),
                        Role::Marker,
                        rect_from_nsrect(rect_in_root_from_flipped_local(
                            self.battlefield_view.frame(),
                            attention_bar,
                        )),
                        card.attention_bar_reason.clone(),
                    ));
                }
                if let Some(attention_bar_reason) = regions.attention_bar_reason {
                    if let Some(reason) = card.attention_bar_reason.clone() {
                        nodes.push(node(
                            &format!("battlefield-card-attention-bar-reason-{}", key.slug()),
                            &selectors::battlefield_card_attention_bar_reason(key),
                            Role::Label,
                            rect_from_nsrect(rect_in_root_from_flipped_local(
                                self.battlefield_view.frame(),
                                attention_bar_reason,
                            )),
                            Some(reason),
                        ));
                    }
                }
                if let Some(scrollback) = regions.scrollback {
                    let scrollback_text = battlefield_view::scrollback_lines(card)
                        .iter()
                        .map(|l: &String| l.trim().to_string())
                        .filter(|l: &String| !l.is_empty())
                        .collect::<Vec<_>>()
                        .join("\n");
                    nodes.push(node(
                        &format!("battlefield-card-scrollback-{}", key.slug()),
                        &selectors::battlefield_card_scrollback(key),
                        Role::Container,
                        rect_from_nsrect(rect_in_root_from_flipped_local(
                            self.battlefield_view.frame(),
                            scrollback,
                        )),
                        if scrollback_text.is_empty() {
                            None
                        } else {
                            Some(scrollback_text)
                        },
                    ));
                }
                if let Some(terminal_slot) = regions.terminal_slot {
                    nodes.push(node(
                        &format!("battlefield-card-terminal-slot-{}", key.slug()),
                        &selectors::battlefield_card_terminal_slot(key),
                        Role::Container,
                        rect_from_nsrect(rect_in_root_from_flipped_local(
                            self.battlefield_view.frame(),
                            terminal_slot,
                        )),
                        None,
                    ));
                }
            }

            nodes
        }

        fn snapshot_recipes(&self) -> Vec<NodeRecipe> {
            let state = self.state.borrow();
            let cards = state.card_render_data();
            let focused = state.workspace.focused_session();
            let embedded_ids =
                workspace_support::embedded_session_ids(&cards, self.root.frame(), focused);
            let selected = state.workspace.selected_session();
            cards
                .iter()
                .zip(battlefield_view::layout_for_mode(
                    cards.len(),
                    self.battlefield_view.frame(),
                    focused.is_some(),
                ))
                .filter_map(|(card, rect)| {
                    let key = session_key(card.id)?;
                    let regions = battlefield_regions(
                        card,
                        &NSRect::new(NSPoint::new(rect.x, rect.y), NSSize::new(rect.w, rect.h)),
                        embedded_ids.contains(&card.id),
                        focused.is_some(),
                    );
                    let card_rect = rect_in_root_from_flipped_local(
                        self.battlefield_view.frame(),
                        NSRect::new(NSPoint::new(rect.x, rect.y), NSSize::new(rect.w, rect.h)),
                    );
                    Some(
                        NodeRecipe::new(
                            selectors::battlefield_card(key),
                            Role::ListItem,
                            RegionSpec::rect(rect_from_nsrect(card_rect)),
                        )
                        .with_selector(selectors::battlefield_card(key))
                        .with_hit_target(RegionSpec::rect(rect_from_nsrect(
                            rect_in_root_from_flipped_local(
                                self.battlefield_view.frame(),
                                regions.hit_target,
                            ),
                        )))
                        .with_label(card.title.clone())
                        .with_state("selected", PropertyValue::Bool(selected == Some(card.id))),
                    )
                })
                .collect()
        }
    }

    fn push_focus_nodes(
        nodes: &mut Vec<SemanticNode>,
        focus_frame: &NSRect,
        key: UiSessionKey,
        data: &FocusRenderData,
    ) {
        let regions = focus_regions(focus_frame, data);
        nodes.push(node(
            &format!("focus-card-{}", key.slug()),
            &selectors::focus_card(key),
            Role::Container,
            rect_from_nsrect(rect_in_root_from_flipped_local(*focus_frame, regions.card)),
            None,
        ));
        if let Some(title) = regions.title {
            nodes.push(node(
                &format!("focus-card-title-{}", key.slug()),
                &selectors::focus_card_title(key),
                Role::Label,
                rect_from_nsrect(rect_in_root_from_flipped_local(*focus_frame, title)),
                Some(data.title.clone()),
            ));
        }
        if let Some(status) = regions.status {
            nodes.push(node(
                &format!("focus-card-status-{}", key.slug()),
                &selectors::focus_card_status(key),
                Role::Label,
                rect_from_nsrect(rect_in_root_from_flipped_local(*focus_frame, status)),
                Some(data.status_label.clone()),
            ));
        }
        if let Some(headline) = regions.headline {
            nodes.push(node(
                &format!("focus-card-headline-{}", key.slug()),
                &selectors::focus_card_headline(key),
                Role::Label,
                rect_from_nsrect(rect_in_root_from_flipped_local(*focus_frame, headline)),
                Some(data.combined_headline.clone()),
            ));
        }
        if let (Some(attention_pill), Some(attention)) = (regions.attention_pill, data.attention) {
            nodes.push(node(
                &format!("focus-card-attention-pill-{}", key.slug()),
                &selectors::focus_card_attention_pill(key),
                Role::Marker,
                rect_from_nsrect(rect_in_root_from_flipped_local(
                    *focus_frame,
                    attention_pill,
                )),
                Some(attention.label.to_string()),
            ));
        }
    }

    fn node(
        id: &str,
        selector: &str,
        role: Role,
        rect: Rect,
        label: Option<String>,
    ) -> SemanticNode {
        let mut node = SemanticNode::new(id, role, rect).with_selector(selector.to_string());
        if let Some(label) = label {
            node = node.with_label(label);
        }
        node
    }

    fn session_key(session_id: SessionId) -> Option<UiSessionKey> {
        match session_id.0 {
            1 => Some(UiSessionKey::Shell1),
            2 => Some(UiSessionKey::Shell2),
            3 => Some(UiSessionKey::Shell3),
            4 => Some(UiSessionKey::Shell4),
            _ => None,
        }
    }

    fn rect_from_nsrect(rect: NSRect) -> Rect {
        Rect::new(
            Point::new(rect.origin.x, rect.origin.y),
            Size::new(rect.size.width, rect.size.height),
        )
    }

    fn rect_in_root_from_flipped_local(view_frame: NSRect, local_rect: NSRect) -> NSRect {
        NSRect::new(
            NSPoint::new(
                view_frame.origin.x + local_rect.origin.x,
                view_frame.origin.y + view_frame.size.height
                    - local_rect.origin.y
                    - local_rect.size.height,
            ),
            local_rect.size,
        )
    }

    #[derive(Clone, Copy)]
    struct BattlefieldRegions {
        title: Option<NSRect>,
        status: Option<NSRect>,
        subtitle: Option<NSRect>,
        headline: Option<NSRect>,
        alert: Option<NSRect>,
        nudge: Option<NSRect>,
        attention_bar: Option<NSRect>,
        attention_bar_reason: Option<NSRect>,
        scrollback: Option<NSRect>,
        terminal_slot: Option<NSRect>,
        hit_target: NSRect,
    }

    fn battlefield_regions(
        card: &CardRenderData,
        card_rect: &NSRect,
        embedded_terminal: bool,
        focused_mode: bool,
    ) -> BattlefieldRegions {
        let chrome = battlefield_view::card_chrome_visibility(card, focused_mode);
        let pad_x = 16.0;
        let pad_y = 14.0;
        let mut y_cursor = card_rect.origin.y + pad_y;
        let content_width = card_rect.size.width - 32.0;
        let header_right_edge = card_rect.origin.x + card_rect.size.width - pad_x;
        let mut regions = BattlefieldRegions {
            title: None,
            status: None,
            subtitle: None,
            headline: None,
            alert: None,
            nudge: None,
            attention_bar: None,
            attention_bar_reason: None,
            scrollback: None,
            terminal_slot: None,
            hit_target: NSRect::new(
                NSPoint::new(
                    card_rect.origin.x + (card_rect.size.width / 2.0) - 2.0,
                    card_rect.origin.y + (card_rect.size.height / 2.0) - 2.0,
                ),
                NSSize::new(4.0, 4.0),
            ),
        };

        // Row 1: Title (left) + Status chip (right-anchored).
        if chrome.title_visible {
            if chrome.status_visible {
                let chip_w = card.status_label.len() as f64 * 7.0 + 16.0;
                let chip_x = header_right_edge - chip_w;
                regions.status = Some(NSRect::new(
                    NSPoint::new(chip_x, y_cursor),
                    NSSize::new(chip_w, 20.0),
                ));
            }
            let title_max_w = if chrome.status_visible {
                let chip_w = card.status_label.len() as f64 * 7.0 + 16.0;
                (header_right_edge - chip_w - 8.0 - (card_rect.origin.x + pad_x)).max(0.0)
            } else {
                content_width
            };
            regions.title = Some(NSRect::new(
                NSPoint::new(card_rect.origin.x + pad_x, y_cursor),
                NSSize::new(title_max_w, 22.0),
            ));
            y_cursor += if focused_mode { 20.0 } else { 24.0 };
        }

        // Row 2: Subtitle/concise headline (left) + Nudge chip (right-anchored).
        if chrome.headline_visible && !card.headline.is_empty() {
            if chrome.nudge_state_visible {
                let nudge_w = card.nudge_state.label.len() as f64 * 6.9 + 18.0;
                let nudge_x = header_right_edge - nudge_w;
                regions.nudge = Some(NSRect::new(
                    NSPoint::new(nudge_x, y_cursor - 2.0),
                    NSSize::new(nudge_w, 22.0),
                ));
            }
            let subtitle_max_w = if chrome.nudge_state_visible {
                let nudge_w = card.nudge_state.label.len() as f64 * 6.9 + 18.0;
                (header_right_edge - nudge_w - 8.0 - (card_rect.origin.x + pad_x)).max(0.0)
            } else {
                content_width
            };
            // subtitle height 18 < title height 22 — encodes typographic subordination.
            let subtitle_rect = NSRect::new(
                NSPoint::new(card_rect.origin.x + pad_x, y_cursor),
                NSSize::new(subtitle_max_w, 18.0),
            );
            regions.subtitle = Some(subtitle_rect);
            regions.headline = Some(subtitle_rect);
            y_cursor += 24.0;
        } else if focused_mode {
            y_cursor += 4.0;
        }

        if embedded_terminal {
            let slot = exaterm_ui::layout::card_terminal_slot_rect(&exaterm_ui::layout::CardRect {
                x: card_rect.origin.x,
                y: card_rect.origin.y,
                w: card_rect.size.width,
                h: card_rect.size.height,
            });
            regions.terminal_slot = Some(NSRect::new(
                NSPoint::new(slot.x, slot.y),
                NSSize::new(slot.w, slot.h),
            ));
            if chrome.bars_visible && card.attention_bar.is_some() {
                let bar_y = (slot.y - 52.0).max(y_cursor);
                let bar_rect = NSRect::new(
                    NSPoint::new(card_rect.origin.x + pad_x, bar_y),
                    NSSize::new(content_width, 56.0),
                );
                regions.attention_bar = Some(bar_rect);
                if card.attention_bar_reason.as_deref().is_some_and(|r| !r.is_empty()) {
                    regions.attention_bar_reason = Some(NSRect::new(
                        NSPoint::new(bar_rect.origin.x, bar_rect.origin.y + 32.0),
                        NSSize::new(content_width, 42.0),
                    ));
                }
            }
            return regions;
        }

        let scrollback_lines = battlefield_view::scrollback_lines(card);
        if !scrollback_lines.is_empty() {
            let scrollback_height = (scrollback_lines.len() as f64 * 18.0) + 16.0;
            regions.scrollback = Some(NSRect::new(
                NSPoint::new(card_rect.origin.x + pad_x, y_cursor),
                NSSize::new(content_width, scrollback_height),
            ));
            y_cursor += scrollback_height + 10.0;
        }

        if chrome.bars_visible && card.attention_bar.is_some() {
            let bar_rect = NSRect::new(
                NSPoint::new(card_rect.origin.x + pad_x, y_cursor),
                NSSize::new(content_width, 56.0),
            );
            regions.attention_bar = Some(bar_rect);
            if card.attention_bar_reason.as_deref().is_some_and(|r| !r.is_empty()) {
                regions.attention_bar_reason = Some(NSRect::new(
                    NSPoint::new(bar_rect.origin.x, bar_rect.origin.y + 32.0),
                    NSSize::new(content_width, 42.0),
                ));
            }
        }

        regions
    }

    struct FocusRegions {
        card: NSRect,
        title: Option<NSRect>,
        status: Option<NSRect>,
        headline: Option<NSRect>,
        attention_pill: Option<NSRect>,
    }

    fn focus_regions(focus_frame: &NSRect, data: &FocusRenderData) -> FocusRegions {
        let card = NSRect::new(
            NSPoint::new(12.0, 0.0),
            NSSize::new(
                (focus_frame.size.width - 24.0).max(0.0),
                focus_frame.size.height,
            ),
        );
        let chrome = exaterm_ui::presentation::chrome_visibility(data.summarized(), true, false);
        let title = chrome.title_visible.then_some(NSRect::new(
            NSPoint::new(card.origin.x + 18.0, card.origin.y + 16.0),
            NSSize::new((card.size.width - 36.0).max(0.0), 24.0),
        ));
        let status = chrome.status_visible.then_some(NSRect::new(
            NSPoint::new(card.origin.x + 18.0, card.origin.y + 44.0),
            NSSize::new((data.status_label.len() as f64 * 7.4) + 18.0, 22.0),
        ));
        let attention_pill = chrome.status_visible.then(|| data.attention).flatten().map(|attention| {
            NSRect::new(
                NSPoint::new(card.origin.x + 158.0, card.origin.y + 44.0),
                NSSize::new((attention.label.len() as f64 * 7.4) + 18.0, 22.0),
            )
        });
        let headline = (chrome.headline_visible && !data.combined_headline.is_empty())
            .then_some(NSRect::new(
                NSPoint::new(card.origin.x + 18.0, card.origin.y + 78.0),
                NSSize::new((card.size.width - 36.0).max(0.0), 56.0),
            ));

        FocusRegions {
            card,
            title,
            status,
            headline,
            attention_pill,
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use exaterm_ui::presentation::NudgeStateTone;
        use exaterm_ui::supervision::BattleCardStatus;

        fn card() -> CardRenderData {
            CardRenderData {
                id: SessionId(1),
                title: "Agent".into(),
                status: BattleCardStatus::Active,
                status_label: "ACTIVE".into(),
                recency: "now".into(),
                scrollback: vec!["$ cargo test".into()],
                headline: "Working".into(),
                combined_headline: "Working".into(),
                detail: None,
                alert: None,
                attention: None,
                attention_reason: None,
                attention_bar: None,
                attention_bar_reason: None,
                nudge_state: exaterm_ui::presentation::NudgeStatePresentation {
                    label: "AUTONUDGE ARMED",
                    css_class: "armed",
                    tone: NudgeStateTone::Armed,
                },
                last_nudge: None,
            }
        }

        #[test]
        fn battlefield_regions_hide_summary_fields_when_unsummarized() {
            let mut sparse = card();
            sparse.headline.clear();
            sparse.combined_headline.clear();
            sparse.scrollback.clear();
            let regions = battlefield_regions(
                &sparse,
                &NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(600.0, 400.0)),
                true,
                false,
            );
            assert!(regions.title.is_none());
            assert!(regions.status.is_none());
            assert!(regions.nudge.is_none());
        }

        #[test]
        fn focus_regions_include_attention_pill_when_present() {
            let data = FocusRenderData {
                id: SessionId(1),
                title: "Agent".into(),
                status: BattleCardStatus::Blocked,
                status_label: "BLOCKED".into(),
                combined_headline: "Waiting".into(),
                attention: Some(exaterm_ui::presentation::AttentionPresentation {
                    label: "TAKEOVER",
                    fill: 5,
                }),
                attention_reason: None,
                attention_bar: None,
                attention_bar_reason: None,
            };
            let regions = focus_regions(
                &NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(800.0, 600.0)),
                &data,
            );
            assert!(regions.title.is_some());
            assert!(regions.status.is_some());
            assert!(regions.attention_pill.is_some());
        }

        #[test]
        fn focus_regions_hide_summary_fields_when_unsummarized() {
            let data = FocusRenderData {
                id: SessionId(1),
                title: "Shell".into(),
                status: BattleCardStatus::Active,
                status_label: "Active".into(),
                combined_headline: String::new(),
                attention: None,
                attention_reason: None,
                attention_bar: None,
                attention_bar_reason: None,
            };
            let regions = focus_regions(
                &NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(800.0, 600.0)),
                &data,
            );
            assert!(regions.title.is_none());
            assert!(regions.status.is_none());
            assert!(regions.headline.is_none());
            assert!(regions.attention_pill.is_none());
        }
    }
}

#[cfg(target_os = "macos")]
pub use imp::*;
