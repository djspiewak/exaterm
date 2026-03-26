use crate::model::SessionLaunch;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkspaceBlueprint {
    pub name: String,
    pub sessions: Vec<SessionLaunch>,
}

impl WorkspaceBlueprint {
    pub fn demo() -> Self {
        Self {
            name: "Built-in Demo Workspace".into(),
            sessions: vec![
                SessionLaunch::shell(
                    "Planner",
                    "Interactive shell",
                    "Planner session ready. Use this tile like a normal terminal.",
                ),
                SessionLaunch::command(
                    "Pulse Stream",
                    "Live output",
                    "/usr/bin/env",
                    vec![
                        "bash".into(),
                        "-lc".into(),
                        "i=1; while true; do printf '[%s] heartbeat %03d\\r\\n' \"$(date +%T)\" \"$i\"; i=$((i+1)); sleep 2; done".into(),
                    ],
                ),
                SessionLaunch::command(
                    "Process View",
                    "Native TUI",
                    "/usr/bin/env",
                    vec!["top".into()],
                ),
                SessionLaunch::shell(
                    "Intervention Shell",
                    "Operator handoff",
                    "Use this shell for direct intervention and experiments.",
                ),
            ],
        }
    }

    pub fn add_shell(number: usize) -> SessionLaunch {
        SessionLaunch::shell(
            format!("Shell {number}"),
            "Generic command session",
            format!("Shell {number} started. This is a real terminal session."),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::WorkspaceBlueprint;

    #[test]
    fn demo_workspace_has_expected_shape() {
        let workspace = WorkspaceBlueprint::demo();

        assert_eq!(workspace.name, "Built-in Demo Workspace");
        assert_eq!(workspace.sessions.len(), 4);
        assert!(workspace
            .sessions
            .iter()
            .any(|session| session.name == "Process View"));
        assert!(workspace
            .sessions
            .iter()
            .any(|session| session.subtitle == "Live output"));
    }

    #[test]
    fn add_shell_uses_generic_command_session_copy() {
        let shell = WorkspaceBlueprint::add_shell(3);

        assert_eq!(shell.name, "Shell 3");
        assert_eq!(shell.subtitle, "Generic command session");
        assert_eq!(shell.program, "/usr/bin/env");
        assert_eq!(shell.args[0], "bash");
    }
}
