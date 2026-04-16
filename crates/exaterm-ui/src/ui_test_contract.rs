use exaterm_core::model::command_launch;
use exaterm_types::model::{SessionEvent, SessionId, SessionKind, SessionLaunch, SessionRecord, SessionStatus};
use exaterm_types::proto::{ObservationSnapshot, SessionSnapshot, WorkspaceSnapshot};
use exaterm_types::synthesis::{AttentionLevel, TacticalState, TacticalSynthesis};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiTestScenario {
    EmptyWorkspace,
    BattlefieldSingleSparse,
    BattlefieldSingleSummarized,
    BattlefieldFourMixed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum UiSessionKey {
    Shell1,
    Shell2,
    Shell3,
    Shell4,
}

impl UiSessionKey {
    pub fn slug(self) -> &'static str {
        match self {
            Self::Shell1 => "shell-1",
            Self::Shell2 => "shell-2",
            Self::Shell3 => "shell-3",
            Self::Shell4 => "shell-4",
        }
    }

    pub const fn session_id(self) -> SessionId {
        match self {
            Self::Shell1 => SessionId(1),
            Self::Shell2 => SessionId(2),
            Self::Shell3 => SessionId(3),
            Self::Shell4 => SessionId(4),
        }
    }
}

#[derive(Clone, Debug)]
pub struct UiScenarioFixture {
    pub snapshot: WorkspaceSnapshot,
    pub selected_session: Option<SessionId>,
    pub window_width: i32,
    pub window_height: i32,
}

pub mod selectors {
    use super::UiSessionKey;

    pub const WORKSPACE_EMPTY_STATE: &str = "workspace.empty-state";
    pub const WORKSPACE_EMPTY_STATE_TITLE: &str = "workspace.empty-state.title";
    pub const WORKSPACE_EMPTY_STATE_BODY: &str = "workspace.empty-state.body";
    pub const WORKSPACE_BATTLEFIELD: &str = "workspace.battlefield";
    pub const WORKSPACE_FOCUS_PANEL: &str = "workspace.focus-panel";

    pub fn battlefield_card(session: UiSessionKey) -> String {
        format!("battlefield.card.{}", session.slug())
    }

    pub fn battlefield_card_title(session: UiSessionKey) -> String {
        format!("{}.title", battlefield_card(session))
    }

    pub fn battlefield_card_status(session: UiSessionKey) -> String {
        format!("{}.status", battlefield_card(session))
    }

    pub fn battlefield_card_headline(session: UiSessionKey) -> String {
        format!("{}.headline", battlefield_card(session))
    }

    pub fn battlefield_card_alert(session: UiSessionKey) -> String {
        format!("{}.alert", battlefield_card(session))
    }

    pub fn battlefield_card_nudge(session: UiSessionKey) -> String {
        format!("{}.nudge", battlefield_card(session))
    }

    pub fn battlefield_card_attention_bar(session: UiSessionKey) -> String {
        format!("{}.attention-bar", battlefield_card(session))
    }

    pub fn battlefield_card_scrollback(session: UiSessionKey) -> String {
        format!("{}.scrollback", battlefield_card(session))
    }

    pub fn battlefield_card_terminal_slot(session: UiSessionKey) -> String {
        format!("{}.terminal-slot", battlefield_card(session))
    }

    pub fn focus_card(session: UiSessionKey) -> String {
        format!("focus.card.{}", session.slug())
    }

    pub fn focus_card_title(session: UiSessionKey) -> String {
        format!("{}.title", focus_card(session))
    }

    pub fn focus_card_status(session: UiSessionKey) -> String {
        format!("{}.status", focus_card(session))
    }

    pub fn focus_card_headline(session: UiSessionKey) -> String {
        format!("{}.headline", focus_card(session))
    }

    pub fn focus_card_attention_pill(session: UiSessionKey) -> String {
        format!("{}.attention-pill", focus_card(session))
    }
}

pub fn scenario_fixture(scenario: UiTestScenario) -> UiScenarioFixture {
    match scenario {
        UiTestScenario::EmptyWorkspace => UiScenarioFixture {
            snapshot: WorkspaceSnapshot::default(),
            selected_session: None,
            window_width: 1480,
            window_height: 960,
        },
        UiTestScenario::BattlefieldSingleSparse => UiScenarioFixture {
            snapshot: WorkspaceSnapshot {
                sessions: vec![session_snapshot(
                    UiSessionKey::Shell1,
                    "Shell 1",
                    "Operator shell",
                    SessionKind::WaitingShell,
                    SessionStatus::Waiting,
                    sparse_observation(&["$ ls", "src", "Cargo.toml"]),
                    None,
                    false,
                    None,
                    None,
                )],
            },
            selected_session: Some(UiSessionKey::Shell1.session_id()),
            window_width: 1480,
            window_height: 960,
        },
        UiTestScenario::BattlefieldSingleSummarized => UiScenarioFixture {
            snapshot: WorkspaceSnapshot {
                sessions: vec![session_snapshot(
                    UiSessionKey::Shell1,
                    "Agent A",
                    "Parser recovery",
                    SessionKind::RunningStream,
                    SessionStatus::Running,
                    observation(
                        &[
                            "• I found the next parser breakage.",
                            "$ cargo test parser_recovery -- --nocapture",
                            "2 parser tests still failing",
                        ],
                        Some("cargo test parser_recovery"),
                        Some("cargo"),
                        vec!["src/parser.rs".into()],
                        Some("test parser::recovery::keeps_trailing_tokens ... FAILED"),
                        9,
                    ),
                    Some(TacticalSynthesis {
                        tactical_state: TacticalState::Working,
                        tactical_state_brief: Some("Parser recovery is still in progress.".into()),
                        attention_level: AttentionLevel::Guide,
                        attention_brief: Some("One failure remains on the recovery path.".into()),
                        headline: Some("Parser recovery narrowed to one failing transition.".into()),
                    }),
                    true,
                    Some("Keep going on the next concrete parser failure.".into()),
                    Some(120),
                )],
            },
            selected_session: Some(UiSessionKey::Shell1.session_id()),
            window_width: 1480,
            window_height: 960,
        },
        UiTestScenario::BattlefieldFourMixed => UiScenarioFixture {
            snapshot: WorkspaceSnapshot {
                sessions: vec![
                    session_snapshot(
                        UiSessionKey::Shell1,
                        "Agent A",
                        "Parser recovery",
                        SessionKind::RunningStream,
                        SessionStatus::Running,
                        observation(
                            &["$ cargo test parser_recovery", "2 parser tests still failing"],
                            Some("cargo test parser_recovery"),
                            Some("cargo"),
                            vec!["src/parser.rs".into()],
                            Some("still narrowing parser failures"),
                            8,
                        ),
                        Some(TacticalSynthesis {
                            tactical_state: TacticalState::Working,
                            tactical_state_brief: Some("Focused parser repair loop.".into()),
                            attention_level: AttentionLevel::Monitor,
                            attention_brief: Some("Active but not urgent.".into()),
                            headline: Some("Parser recovery progressing on the focused suite.".into()),
                        }),
                        true,
                        Some("Autonudge is armed for the next checkpoint.".into()),
                        None,
                    ),
                    session_snapshot(
                        UiSessionKey::Shell2,
                        "Agent B",
                        "Deploy approval",
                        SessionKind::BlockingPrompt,
                        SessionStatus::Blocked,
                        observation(
                            &[
                                "Proceed with deploy? [y/N]",
                                "Waiting for approval.",
                            ],
                            Some("bash"),
                            Some("bash"),
                            vec!["deploy/prod.sh".into()],
                            Some("prompting for deploy approval"),
                            75,
                        ),
                        Some(TacticalSynthesis {
                            tactical_state: TacticalState::Blocked,
                            tactical_state_brief: Some("Waiting on explicit operator approval.".into()),
                            attention_level: AttentionLevel::Takeover,
                            attention_brief: Some("Human intervention is required before deploy can continue.".into()),
                            headline: Some("Blocked on production deploy approval.".into()),
                        }),
                        false,
                        None,
                        None,
                    ),
                    session_snapshot(
                        UiSessionKey::Shell3,
                        "Shell 3",
                        "Operator shell",
                        SessionKind::WaitingShell,
                        SessionStatus::Waiting,
                        sparse_observation(&["$ git status", "On branch main"]),
                        None,
                        false,
                        None,
                        None,
                    ),
                    session_snapshot(
                        UiSessionKey::Shell4,
                        "Agent D",
                        "Post-fix watch",
                        SessionKind::PlanningStream,
                        SessionStatus::Waiting,
                        observation(
                            &["• Stable. Standing by.", "• Still stable; waiting for the next instruction."],
                            None,
                            None,
                            Vec::new(),
                            None,
                            210,
                        ),
                        Some(TacticalSynthesis {
                            tactical_state: TacticalState::Stopped,
                            tactical_state_brief: Some("Clean checkpoint, waiting for a nudge.".into()),
                            attention_level: AttentionLevel::Guide,
                            attention_brief: Some("Ready for the next pass.".into()),
                            headline: Some("Checkpoint complete and waiting for direction.".into()),
                        }),
                        true,
                        Some("Keep going on the next concrete failure.".into()),
                        Some(35),
                    ),
                ],
            },
            selected_session: Some(UiSessionKey::Shell2.session_id()),
            window_width: 1200,
            window_height: 900,
        },
    }
}

fn session_snapshot(
    key: UiSessionKey,
    name: &str,
    subtitle: &str,
    kind: SessionKind,
    status: SessionStatus,
    observation: ObservationSnapshot,
    summary: Option<TacticalSynthesis>,
    auto_nudge_enabled: bool,
    last_nudge: Option<String>,
    last_sent_age_secs: Option<u64>,
) -> SessionSnapshot {
    SessionSnapshot {
        record: SessionRecord {
            id: key.session_id(),
            launch: fixture_launch(name, subtitle, kind),
            display_name: Some(name.into()),
            status,
            pid: Some(1000 + key.session_id().0),
            events: vec![SessionEvent {
                sequence: 1,
                summary: "fixture session".into(),
            }],
        },
        observation,
        summary,
        raw_stream_socket_name: None,
        auto_nudge_enabled,
        last_nudge,
        last_sent_age_secs,
    }
}

fn fixture_launch(name: &str, subtitle: &str, kind: SessionKind) -> SessionLaunch {
    command_launch(name, subtitle, kind, "/usr/bin/env", vec!["true".into()])
}

fn sparse_observation(lines: &[&str]) -> ObservationSnapshot {
    observation(lines, None, None, Vec::new(), None, 14)
}

fn observation(
    lines: &[&str],
    active_command: Option<&str>,
    dominant_process: Option<&str>,
    recent_files: Vec<String>,
    work_output_excerpt: Option<&str>,
    last_change_age_secs: u64,
) -> ObservationSnapshot {
    ObservationSnapshot {
        last_change_age_secs,
        recent_lines: lines.iter().map(|line| (*line).to_string()).collect(),
        painted_line: lines.last().map(|line| (*line).to_string()),
        shell_child_command: active_command.map(str::to_string),
        active_command: active_command.map(str::to_string),
        dominant_process: dominant_process.map(str::to_string),
        process_tree_excerpt: dominant_process.map(|process| format!("{process} --fixture")),
        recent_files,
        work_output_excerpt: work_output_excerpt.map(str::to_string),
    }
}

#[cfg(test)]
mod tests {
    use super::{scenario_fixture, selectors, UiSessionKey, UiTestScenario};

    #[test]
    fn selectors_use_stable_human_readable_names() {
        assert_eq!(selectors::WORKSPACE_EMPTY_STATE, "workspace.empty-state");
        assert_eq!(
            selectors::battlefield_card_title(UiSessionKey::Shell1),
            "battlefield.card.shell-1.title"
        );
        assert_eq!(
            selectors::focus_card_attention_pill(UiSessionKey::Shell4),
            "focus.card.shell-4.attention-pill"
        );
    }

    #[test]
    fn four_mixed_fixture_stays_selected_on_shell_two() {
        let fixture = scenario_fixture(UiTestScenario::BattlefieldFourMixed);
        assert_eq!(fixture.selected_session, Some(UiSessionKey::Shell2.session_id()));
        assert_eq!(fixture.snapshot.sessions.len(), 4);
    }
}
