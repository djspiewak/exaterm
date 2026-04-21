#[cfg(target_os = "linux")]
mod imp {
    use std::rc::Rc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use exaterm_types::model::SessionId;
    use exaterm_ui::ui_test_contract::{scenario_fixture, selectors, UiSessionKey, UiTestScenario};
    use glasscheck::{
        Harness, InstrumentedNode, Point, PropertyValue, Rect, Role, SceneSource, SemanticNode,
        Size, WindowHost,
    };
    use gtk::gio;
    use gtk::prelude::*;
    use libadwaita as adw;

    use crate::ui::{
        activate_battlefield_card, apply_workspace_snapshot, build_test_ui,
        refresh_snapshot_and_cards, AppContext, BuiltTestUi,
    };

    static NEXT_APP_ID: AtomicUsize = AtomicUsize::new(1);

    #[derive(Debug)]
    pub struct MountError {
        message: String,
    }

    impl MountError {
        pub(crate) fn new(message: impl Into<String>) -> Self {
            Self {
                message: message.into(),
            }
        }
    }

    impl std::fmt::Display for MountError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", self.message)
        }
    }

    impl std::error::Error for MountError {}

    pub struct MountedGtkUi {
        pub host: WindowHost,
    }

    pub fn mount_scenario(
        harness: &Harness,
        scenario: UiTestScenario,
    ) -> Result<MountedGtkUi, MountError> {
        adw::init().map_err(|error| MountError::new(error.to_string()))?;
        adw::StyleManager::default().set_color_scheme(adw::ColorScheme::ForceDark);

        let fixture = scenario_fixture(scenario);
        let app_id = format!(
            "io.exaterm.exaterm.glasscheck.test{}",
            NEXT_APP_ID.fetch_add(1, Ordering::Relaxed)
        );
        let app = gtk::Application::builder()
            .application_id(&app_id)
            .flags(gio::ApplicationFlags::NON_UNIQUE)
            .build();
        app.register(None::<&gio::Cancellable>)
            .map_err(|error| MountError::new(error.to_string()))?;

        let built = build_test_ui(&app, exaterm_ui::beachhead::RunMode::Local);
        built
            .window
            .set_default_size(fixture.window_width, fixture.window_height);

        built.window.present();
        let host = WindowHost::from_root(&built.body, Some(built.window.upcast_ref()));
        host.set_scene_source(Box::new(GtkSceneSource::new(&built, built.context.clone())));
        harness.settle(4);

        apply_workspace_snapshot(&built.context, fixture.snapshot);
        if let Some(session_id) = fixture.selected_session {
            built.context.state.borrow_mut().select_session(session_id);
            let row = built
                .context
                .session_cards
                .borrow()
                .get(&session_id)
                .map(|card| card.row.clone());
            if let Some(row) = row {
                built.context.cards.select_child(&row);
            }
        }
        refresh_snapshot_and_cards(&built.context);
        register_interaction_nodes(&host, &built.context);
        harness.settle(8);

        Ok(MountedGtkUi { host })
    }

    fn register_interaction_nodes(host: &WindowHost, context: &Rc<AppContext>) {
        for (session_id, card) in context.session_cards.borrow().iter() {
            let session_id = *session_id;
            let Some(key) = session_key(session_id) else {
                continue;
            };
            let click = gtk::GestureClick::new();
            click.set_button(1);
            let row = card.row.clone();
            let context = context.clone();
            click.connect_released(move |_, _, _, _| {
                activate_battlefield_card(&context, &row, session_id);
            });
            card.row.add_controller(click);
            host.register_node(
                &card.row,
                InstrumentedNode {
                    id: Some(selectors::battlefield_card(key)),
                    role: Some(Role::ListItem),
                    label: Some(card.title.label().to_string()),
                },
            );
        }
    }

    struct GtkSceneSource {
        root: gtk::Widget,
        empty_title: gtk::Label,
        empty_body: gtk::Label,
        context: Rc<AppContext>,
    }

    impl GtkSceneSource {
        fn new(built: &BuiltTestUi, context: Rc<AppContext>) -> Self {
            Self {
                root: built.body.clone().upcast(),
                empty_title: built.empty_title.clone(),
                empty_body: built.empty_body.clone(),
                context,
            }
        }
    }

    impl SceneSource for GtkSceneSource {
        fn snapshot_nodes(&self) -> Vec<SemanticNode> {
            let mut nodes = Vec::new();

            if self.context.empty_state.is_visible() {
                push_widget_node(
                    &mut nodes,
                    &self.root,
                    "workspace-empty-state",
                    selectors::WORKSPACE_EMPTY_STATE,
                    Role::Placeholder,
                    &self.context.empty_state,
                    None,
                );
                push_widget_node(
                    &mut nodes,
                    &self.root,
                    "workspace-empty-state-title",
                    selectors::WORKSPACE_EMPTY_STATE_TITLE,
                    Role::Label,
                    &self.empty_title,
                    Some(self.empty_title.label().to_string()),
                );
                push_widget_node(
                    &mut nodes,
                    &self.root,
                    "workspace-empty-state-body",
                    selectors::WORKSPACE_EMPTY_STATE_BODY,
                    Role::Label,
                    &self.empty_body,
                    Some(self.empty_body.label().to_string()),
                );
            }

            if self.context.battlefield_panel.is_visible() {
                push_widget_node(
                    &mut nodes,
                    &self.root,
                    "workspace-battlefield",
                    selectors::WORKSPACE_BATTLEFIELD,
                    Role::List,
                    &self.context.battlefield_panel,
                    None,
                );
            }

            if self.context.focus.panel.is_visible() {
                push_widget_node(
                    &mut nodes,
                    &self.root,
                    "workspace-focus-panel",
                    selectors::WORKSPACE_FOCUS_PANEL,
                    Role::Container,
                    &self.context.focus.panel,
                    None,
                );
            }

            let selected = self.context.state.borrow().selected_session();
            for (session_id, card) in self.context.session_cards.borrow().iter() {
                let Some(key) = session_key(*session_id) else {
                    continue;
                };
                if card.row.is_visible() {
                    let mut node = widget_node(
                        &self.root,
                        &format!("battlefield-card-{}", key.slug()),
                        &selectors::battlefield_card(key),
                        Role::ListItem,
                        &card.frame,
                        None,
                    );
                    node = node.with_state(
                        "selected",
                        PropertyValue::Bool(selected == Some(*session_id)),
                    );
                    nodes.push(node);
                }
                if card.title.is_visible() {
                    nodes.push(widget_node(
                        &self.root,
                        &format!("battlefield-card-title-{}", key.slug()),
                        &selectors::battlefield_card_title(key),
                        Role::Label,
                        &card.title,
                        Some(card.title.label().to_string()),
                    ));
                }
                if card.status.is_visible() {
                    nodes.push(widget_node(
                        &self.root,
                        &format!("battlefield-card-status-{}", key.slug()),
                        &selectors::battlefield_card_status(key),
                        Role::Label,
                        &card.status,
                        Some(card.status.label().to_string()),
                    ));
                }
                if card.headline.is_visible() {
                    nodes.push(widget_node(
                        &self.root,
                        &format!("battlefield-card-headline-{}", key.slug()),
                        &selectors::battlefield_card_headline(key),
                        Role::Label,
                        &card.headline,
                        Some(card.headline.label().to_string()),
                    ));
                }
                if card.alert.is_visible() {
                    nodes.push(widget_node(
                        &self.root,
                        &format!("battlefield-card-alert-{}", key.slug()),
                        &selectors::battlefield_card_alert(key),
                        Role::Label,
                        &card.alert,
                        Some(card.alert.label().to_string()),
                    ));
                }
                if card.nudge_state.is_visible() {
                    nodes.push(widget_node(
                        &self.root,
                        &format!("battlefield-card-nudge-{}", key.slug()),
                        &selectors::battlefield_card_nudge(key),
                        Role::Label,
                        &card.nudge_state,
                        Some(card.nudge_state.label().to_string()),
                    ));
                }
                if card.momentum_bar.frame.is_visible() {
                    nodes.push(widget_node(
                        &self.root,
                        &format!("battlefield-card-attention-bar-{}", key.slug()),
                        &selectors::battlefield_card_attention_bar(key),
                        Role::Marker,
                        &card.momentum_bar.frame,
                        Some(card.momentum_bar.reason.label().to_string()),
                    ));
                }
                let visible_child = card.middle_stack.visible_child_name();
                if card.middle_stack.is_visible() && visible_child.as_deref() == Some("scrollback")
                {
                    nodes.push(widget_node(
                        &self.root,
                        &format!("battlefield-card-scrollback-{}", key.slug()),
                        &selectors::battlefield_card_scrollback(key),
                        Role::Container,
                        &card.scrollback_band,
                        None,
                    ));
                }
                if card.middle_stack.is_visible() && visible_child.as_deref() == Some("terminal") {
                    nodes.push(widget_node(
                        &self.root,
                        &format!("battlefield-card-terminal-slot-{}", key.slug()),
                        &selectors::battlefield_card_terminal_slot(key),
                        Role::Container,
                        &card.terminal_slot,
                        None,
                    ));
                }
            }

            if let Some(focused) = self.context.state.borrow().focused_session() {
                if let Some(key) = session_key(focused) {
                    push_widget_node(
                        &mut nodes,
                        &self.root,
                        &format!("focus-card-{}", key.slug()),
                        &selectors::focus_card(key),
                        Role::Container,
                        &self.context.focus.frame,
                        None,
                    );
                    if self.context.focus.title.is_visible() {
                        push_widget_node(
                            &mut nodes,
                            &self.root,
                            &format!("focus-card-title-{}", key.slug()),
                            &selectors::focus_card_title(key),
                            Role::Label,
                            &self.context.focus.title,
                            Some(self.context.focus.title.label().to_string()),
                        );
                    }
                    if self.context.focus.status.is_visible() {
                        push_widget_node(
                            &mut nodes,
                            &self.root,
                            &format!("focus-card-status-{}", key.slug()),
                            &selectors::focus_card_status(key),
                            Role::Label,
                            &self.context.focus.status,
                            Some(self.context.focus.status.label().to_string()),
                        );
                    }
                    if self.context.focus.headline.is_visible() {
                        push_widget_node(
                            &mut nodes,
                            &self.root,
                            &format!("focus-card-headline-{}", key.slug()),
                            &selectors::focus_card_headline(key),
                            Role::Label,
                            &self.context.focus.headline,
                            Some(self.context.focus.headline.label().to_string()),
                        );
                    }
                    if self.context.focus.attention_pill.is_visible() {
                        push_widget_node(
                            &mut nodes,
                            &self.root,
                            &format!("focus-card-attention-pill-{}", key.slug()),
                            &selectors::focus_card_attention_pill(key),
                            Role::Marker,
                            &self.context.focus.attention_pill,
                            Some(self.context.focus.attention_pill.label().to_string()),
                        );
                    }
                }
            }

            nodes
        }
    }

    fn push_widget_node(
        nodes: &mut Vec<SemanticNode>,
        root: &gtk::Widget,
        id: &str,
        selector: &str,
        role: Role,
        widget: &impl IsA<gtk::Widget>,
        label: Option<String>,
    ) {
        nodes.push(widget_node(root, id, selector, role, widget, label));
    }

    fn widget_node(
        root: &gtk::Widget,
        id: &str,
        selector: &str,
        role: Role,
        widget: &impl IsA<gtk::Widget>,
        label: Option<String>,
    ) -> SemanticNode {
        let rect = widget_rect(root, widget.as_ref())
            .unwrap_or_else(|| Rect::new(Point::new(0.0, 0.0), Size::new(0.0, 0.0)));
        let mut node = SemanticNode::new(id, role, rect).with_selector(selector.to_string());
        if let Some(label) = label {
            node = node.with_label(label);
        }
        node
    }

    fn widget_rect(root: &gtk::Widget, widget: &gtk::Widget) -> Option<Rect> {
        if !widget.is_visible() {
            return None;
        }
        let bounds = widget.compute_bounds(root)?;
        let root_height = root.allocated_height().max(1) as f64;
        Some(Rect::new(
            Point::new(
                bounds.x() as f64,
                root_height - bounds.y() as f64 - bounds.height() as f64,
            ),
            Size::new(bounds.width() as f64, bounds.height() as f64),
        ))
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
}

#[cfg(target_os = "linux")]
pub use imp::*;
