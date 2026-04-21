use std::collections::BTreeSet;

use exaterm_types::model::SessionId;
use objc2_foundation::{NSPoint, NSRect, NSSize};

use crate::app_state::{AppState, CardRenderData};

pub const FOCUS_RAIL_HEIGHT: f64 = 240.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BattlefieldActivation {
    Focused(SessionId),
    ReturnedToBattlefield,
    SelectedEmbedded(SessionId),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorkspaceLayout {
    pub empty_state_visible: bool,
    pub battlefield_visible: bool,
    pub focus_visible: bool,
    pub battlefield_frame: NSRect,
    pub focus_frame: NSRect,
}

pub fn workspace_layout(
    content_frame: NSRect,
    has_sessions: bool,
    focused: Option<SessionId>,
) -> WorkspaceLayout {
    if !has_sessions {
        return WorkspaceLayout {
            empty_state_visible: true,
            battlefield_visible: false,
            focus_visible: false,
            battlefield_frame: content_frame,
            focus_frame: NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(0.0, 0.0)),
        };
    }

    if focused.is_some() {
        let rail_height = content_frame.size.height.min(FOCUS_RAIL_HEIGHT);
        let focus_height = (content_frame.size.height - rail_height).max(0.0);
        return WorkspaceLayout {
            empty_state_visible: false,
            battlefield_visible: true,
            focus_visible: true,
            battlefield_frame: NSRect::new(
                NSPoint::new(0.0, focus_height),
                NSSize::new(content_frame.size.width, rail_height),
            ),
            focus_frame: NSRect::new(
                NSPoint::new(0.0, 0.0),
                NSSize::new(content_frame.size.width, focus_height),
            ),
        };
    }

    WorkspaceLayout {
        empty_state_visible: false,
        battlefield_visible: true,
        focus_visible: false,
        battlefield_frame: content_frame,
        focus_frame: NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(0.0, 0.0)),
    }
}

pub fn embedded_session_ids(
    cards: &[CardRenderData],
    content_frame: NSRect,
    focused: Option<SessionId>,
) -> BTreeSet<SessionId> {
    if focused.is_some() || cards.is_empty() {
        return BTreeSet::new();
    }

    let width = content_frame.size.width as i32;
    let height = content_frame.size.height as i32;
    let columns =
        exaterm_ui::layout::battlefield_columns(cards.len(), width, false).max(1) as usize;
    if !exaterm_ui::layout::battlefield_can_embed_terminals(cards.len(), columns, width, height) {
        return BTreeSet::new();
    }

    cards.iter().map(|card| card.id).collect()
}

pub fn activate_battlefield_session(
    state: &mut AppState,
    content_frame: NSRect,
    session_id: SessionId,
) -> BattlefieldActivation {
    let focused_before = state.workspace.focused_session();
    let embedded_ids = embedded_session_ids(&state.card_render_data(), content_frame, None);

    if let Some(focused_session) = focused_before {
        if focused_session == session_id {
            state.workspace.return_to_battlefield();
            return BattlefieldActivation::ReturnedToBattlefield;
        } else {
            state.workspace.enter_focus_mode(session_id);
            return BattlefieldActivation::Focused(session_id);
        }
    }

    if embedded_ids.contains(&session_id) {
        state.workspace.select_session(session_id);
        BattlefieldActivation::SelectedEmbedded(session_id)
    } else {
        state.workspace.enter_focus_mode(session_id);
        BattlefieldActivation::Focused(session_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use exaterm_ui::ui_test_contract::{scenario_fixture, UiSessionKey, UiTestScenario};

    fn frame(width: f64, height: f64) -> NSRect {
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(width, height))
    }

    #[test]
    fn empty_layout_hides_battlefield_and_focus() {
        let layout = workspace_layout(frame(1200.0, 900.0), false, None);
        assert!(layout.empty_state_visible);
        assert!(!layout.battlefield_visible);
        assert!(!layout.focus_visible);
    }

    #[test]
    fn focused_layout_splits_battlefield_and_focus_frames() {
        let layout = workspace_layout(frame(1200.0, 900.0), true, Some(SessionId(2)));
        assert!(layout.battlefield_visible);
        assert!(layout.focus_visible);
        assert_eq!(layout.battlefield_frame.size.height, 240.0);
        assert_eq!(layout.focus_frame.size.height, 660.0);
    }

    #[test]
    fn activation_enters_focus_for_selected_scrollback_card() {
        let fixture = scenario_fixture(UiTestScenario::BattlefieldFourMixed);
        let mut state = AppState::new();
        state.apply_snapshot(&fixture.snapshot);
        state
            .workspace
            .select_session(UiSessionKey::Shell2.session_id());

        let activation = activate_battlefield_session(
            &mut state,
            frame(fixture.window_width as f64, fixture.window_height as f64),
            UiSessionKey::Shell2.session_id(),
        );

        assert_eq!(
            activation,
            BattlefieldActivation::Focused(UiSessionKey::Shell2.session_id())
        );
        assert_eq!(
            state.workspace.focused_session(),
            Some(UiSessionKey::Shell2.session_id())
        );
    }

    #[test]
    fn activation_enters_focus_for_unselected_scrollback_card() {
        let fixture = scenario_fixture(UiTestScenario::BattlefieldFourMixed);
        let mut state = AppState::new();
        state.apply_snapshot(&fixture.snapshot);
        state
            .workspace
            .select_session(UiSessionKey::Shell2.session_id());

        let activation = activate_battlefield_session(
            &mut state,
            frame(fixture.window_width as f64, fixture.window_height as f64),
            UiSessionKey::Shell1.session_id(),
        );

        assert_eq!(
            activation,
            BattlefieldActivation::Focused(UiSessionKey::Shell1.session_id())
        );
        assert_eq!(
            state.workspace.focused_session(),
            Some(UiSessionKey::Shell1.session_id())
        );
        assert_eq!(
            state.workspace.selected_session(),
            Some(UiSessionKey::Shell1.session_id())
        );
    }

    #[test]
    fn activation_does_not_enter_focus_for_selected_embedded_card() {
        let fixture = scenario_fixture(UiTestScenario::BattlefieldSingleSparse);
        let mut state = AppState::new();
        state.apply_snapshot(&fixture.snapshot);
        state
            .workspace
            .select_session(UiSessionKey::Shell1.session_id());

        let activation = activate_battlefield_session(
            &mut state,
            frame(fixture.window_width as f64, fixture.window_height as f64),
            UiSessionKey::Shell1.session_id(),
        );

        assert_eq!(
            activation,
            BattlefieldActivation::SelectedEmbedded(UiSessionKey::Shell1.session_id())
        );
        assert_eq!(state.workspace.focused_session(), None);
    }

    #[test]
    fn activation_returns_to_battlefield_for_focused_session() {
        let fixture = scenario_fixture(UiTestScenario::BattlefieldFourMixed);
        let mut state = AppState::new();
        state.apply_snapshot(&fixture.snapshot);
        state
            .workspace
            .enter_focus_mode(UiSessionKey::Shell2.session_id());

        let activation = activate_battlefield_session(
            &mut state,
            frame(fixture.window_width as f64, fixture.window_height as f64),
            UiSessionKey::Shell2.session_id(),
        );

        assert_eq!(activation, BattlefieldActivation::ReturnedToBattlefield);
        assert_eq!(state.workspace.focused_session(), None);
        assert_eq!(
            state.workspace.selected_session(),
            Some(UiSessionKey::Shell2.session_id())
        );
    }

    #[test]
    fn activation_switches_focus_to_different_session_in_focus_mode() {
        let fixture = scenario_fixture(UiTestScenario::BattlefieldFourMixed);
        let mut state = AppState::new();
        state.apply_snapshot(&fixture.snapshot);
        state
            .workspace
            .enter_focus_mode(UiSessionKey::Shell2.session_id());

        let activation = activate_battlefield_session(
            &mut state,
            frame(fixture.window_width as f64, fixture.window_height as f64),
            UiSessionKey::Shell4.session_id(),
        );

        assert_eq!(
            activation,
            BattlefieldActivation::Focused(UiSessionKey::Shell4.session_id())
        );
        assert_eq!(
            state.workspace.focused_session(),
            Some(UiSessionKey::Shell4.session_id())
        );
        assert_eq!(
            state.workspace.selected_session(),
            Some(UiSessionKey::Shell4.session_id())
        );
    }
}
