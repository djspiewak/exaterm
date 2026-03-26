use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SessionId(pub u32);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionLaunch {
    pub name: String,
    pub subtitle: String,
    pub program: String,
    pub args: Vec<String>,
    pub cwd: Option<PathBuf>,
}

impl SessionLaunch {
    pub fn shell(
        name: impl Into<String>,
        subtitle: impl Into<String>,
        banner: impl Into<String>,
    ) -> Self {
        let banner = banner.into().replace('\'', r"'\''");
        Self {
            name: name.into(),
            subtitle: subtitle.into(),
            program: "/usr/bin/env".into(),
            args: vec![
                "bash".into(),
                "-lc".into(),
                format!("printf '%s\\r\\n' '{banner}'; exec bash -i"),
            ],
            cwd: None,
        }
    }

    pub fn command(
        name: impl Into<String>,
        subtitle: impl Into<String>,
        program: impl Into<String>,
        args: Vec<String>,
    ) -> Self {
        Self {
            name: name.into(),
            subtitle: subtitle.into(),
            program: program.into(),
            args,
            cwd: None,
        }
    }

    pub fn argv(&self) -> Vec<String> {
        std::iter::once(self.program.clone())
            .chain(self.args.iter().cloned())
            .collect()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionStatus {
    Launching,
    Live,
    Exited(i32),
}

impl SessionStatus {
    pub fn chip_label(self) -> String {
        match self {
            SessionStatus::Launching => "Launching".into(),
            SessionStatus::Live => "Live".into(),
            SessionStatus::Exited(code) if code == 0 => "Exited".into(),
            SessionStatus::Exited(code) => format!("Exit {code}"),
        }
    }

    pub fn css_class(self) -> &'static str {
        match self {
            SessionStatus::Launching => "status-launching",
            SessionStatus::Live => "status-live",
            SessionStatus::Exited(0) => "status-exited-clean",
            SessionStatus::Exited(_) => "status-exited-error",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionRecord {
    pub id: SessionId,
    pub launch: SessionLaunch,
    pub status: SessionStatus,
    pub pid: Option<u32>,
}

#[derive(Debug, Default)]
pub struct WorkspaceState {
    next_session_id: u32,
    sessions: Vec<SessionRecord>,
    selected_session: Option<SessionId>,
    focused_terminal: Option<SessionId>,
}

impl WorkspaceState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn load_workspace(&mut self, launches: Vec<SessionLaunch>) -> Vec<SessionId> {
        self.next_session_id = 1;
        self.sessions.clear();
        self.selected_session = None;
        self.focused_terminal = None;

        let mut ids = Vec::with_capacity(launches.len());
        for launch in launches {
            ids.push(self.add_session(launch));
        }
        ids
    }

    pub fn add_session(&mut self, launch: SessionLaunch) -> SessionId {
        let id = SessionId(self.next_session_id);
        self.next_session_id += 1;

        self.sessions.push(SessionRecord {
            id,
            launch,
            status: SessionStatus::Launching,
            pid: None,
        });

        self.selected_session.get_or_insert(id);
        id
    }

    pub fn sessions(&self) -> &[SessionRecord] {
        &self.sessions
    }

    pub fn selected_session(&self) -> Option<SessionId> {
        self.selected_session
    }

    pub fn focused_terminal(&self) -> Option<SessionId> {
        self.focused_terminal
    }

    pub fn select_session(&mut self, session_id: SessionId) {
        if self.sessions.iter().any(|session| session.id == session_id) {
            self.selected_session = Some(session_id);
        }
    }

    pub fn set_terminal_focus(&mut self, session_id: Option<SessionId>) {
        self.focused_terminal =
            session_id.filter(|id| self.sessions.iter().any(|session| session.id == *id));
    }

    pub fn mark_spawned(&mut self, session_id: SessionId, pid: u32) {
        if let Some(session) = self
            .sessions
            .iter_mut()
            .find(|session| session.id == session_id)
        {
            session.status = SessionStatus::Live;
            session.pid = Some(pid);
        }
    }

    pub fn mark_exited(&mut self, session_id: SessionId, exit_code: i32) {
        if let Some(session) = self
            .sessions
            .iter_mut()
            .find(|session| session.id == session_id)
        {
            session.status = SessionStatus::Exited(exit_code);
            session.pid = None;
            if self.focused_terminal == Some(session_id) {
                self.focused_terminal = None;
            }
        }
    }

    pub fn tile_position(index: usize, columns: usize) -> (i32, i32) {
        let columns = columns.max(1);
        let row = index / columns;
        let col = index % columns;
        (col as i32, row as i32)
    }
}

#[cfg(test)]
mod tests {
    use super::{SessionLaunch, SessionStatus, WorkspaceState};

    #[test]
    fn loading_workspace_selects_first_session() {
        let mut state = WorkspaceState::new();
        let ids = state.load_workspace(vec![
            SessionLaunch::shell("One", "shell", "banner"),
            SessionLaunch::shell("Two", "shell", "banner"),
        ]);

        assert_eq!(state.sessions().len(), 2);
        assert_eq!(state.selected_session(), Some(ids[0]));
        assert_eq!(state.sessions()[0].status, SessionStatus::Launching);
    }

    #[test]
    fn add_session_preserves_existing_selection() {
        let mut state = WorkspaceState::new();
        let first = state.add_session(SessionLaunch::shell("One", "shell", "banner"));
        let second = state.add_session(SessionLaunch::shell("Two", "shell", "banner"));

        assert_eq!(state.selected_session(), Some(first));
        assert_eq!(state.sessions()[1].id, second);
    }

    #[test]
    fn spawn_and_exit_updates_runtime_state() {
        let mut state = WorkspaceState::new();
        let first = state.add_session(SessionLaunch::shell("One", "shell", "banner"));

        state.mark_spawned(first, 4242);
        assert_eq!(state.sessions()[0].status, SessionStatus::Live);
        assert_eq!(state.sessions()[0].pid, Some(4242));

        state.set_terminal_focus(Some(first));
        state.mark_exited(first, 1);
        assert_eq!(state.sessions()[0].status, SessionStatus::Exited(1));
        assert_eq!(state.focused_terminal(), None);
    }

    #[test]
    fn tile_positions_fill_rows_left_to_right() {
        assert_eq!(WorkspaceState::tile_position(0, 2), (0, 0));
        assert_eq!(WorkspaceState::tile_position(1, 2), (1, 0));
        assert_eq!(WorkspaceState::tile_position(2, 2), (0, 1));
        assert_eq!(WorkspaceState::tile_position(5, 3), (2, 1));
    }
}
