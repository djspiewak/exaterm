use crate::demo::WorkspaceBlueprint;
use crate::model::{SessionId, SessionLaunch, SessionStatus, WorkspaceState};
use gtk::gdk;
use gtk::prelude::*;
use libadwaita as adw;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use vte::prelude::*;
use vte4 as vte;

const TILE_COLUMNS: usize = 2;
const APP_ID: &str = "io.exaterm.Exaterm";

#[derive(Clone)]
struct SessionTileWidgets {
    root: gtk::Frame,
    status: gtk::Label,
    terminal: vte::Terminal,
}

struct AppContext {
    state: Rc<RefCell<WorkspaceState>>,
    grid: gtk::Grid,
    title: adw::WindowTitle,
    tiles: RefCell<BTreeMap<SessionId, SessionTileWidgets>>,
}

pub fn run() -> glib::ExitCode {
    let app = gtk::Application::builder().application_id(APP_ID).build();
    app.connect_startup(|_| {
        adw::init().expect("libadwaita should initialize");
        adw::StyleManager::default().set_color_scheme(adw::ColorScheme::ForceDark);
    });
    app.connect_activate(build_ui);
    app.run()
}

fn build_ui(app: &gtk::Application) {
    load_css();

    let context = Rc::new(AppContext {
        state: Rc::new(RefCell::new(WorkspaceState::new())),
        grid: gtk::Grid::builder()
            .column_spacing(12)
            .row_spacing(12)
            .margin_top(12)
            .margin_bottom(12)
            .margin_start(12)
            .margin_end(12)
            .hexpand(true)
            .vexpand(true)
            .build(),
        title: adw::WindowTitle::new("Exaterm", "Grid-first, detail-on-demand"),
        tiles: RefCell::new(BTreeMap::new()),
    });

    let add_shell_button = gtk::Button::builder()
        .label("Add Shell")
        .css_classes(vec!["pill".to_string()])
        .tooltip_text("Open a new generic command session")
        .build();

    let header = adw::HeaderBar::builder()
        .title_widget(&context.title)
        .show_end_title_buttons(true)
        .build();
    header.pack_end(&add_shell_button);

    let scroller = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&context.grid)
        .hexpand(true)
        .vexpand(true)
        .build();

    let body = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .build();
    body.append(&header);
    body.append(&scroller);

    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("Exaterm")
        .default_width(1440)
        .default_height(920)
        .content(&body)
        .build();

    {
        let context = context.clone();
        add_shell_button.connect_clicked(move |_| {
            let number = context.state.borrow().sessions().len() + 1;
            let launch = WorkspaceBlueprint::add_shell(number);
            append_session_tile(&context, launch);
        });
    }

    for launch in WorkspaceBlueprint::demo().sessions {
        append_session_tile(&context, launch);
    }
    refresh_window_title(&context);
    refresh_tile_styles(&context);

    window.present();
}

fn append_session_tile(context: &Rc<AppContext>, launch: SessionLaunch) {
    let session_id = context.state.borrow_mut().add_session(launch);
    let session = context
        .state
        .borrow()
        .sessions()
        .iter()
        .find(|session| session.id == session_id)
        .cloned()
        .expect("newly added session must exist");

    let tile = build_tile(context, session_id, &session.launch, session.status);

    let index = context.tiles.borrow().len();
    let (column, row) = WorkspaceState::tile_position(index, TILE_COLUMNS);
    context.grid.attach(&tile.root, column, row, 1, 1);
    context.tiles.borrow_mut().insert(session_id, tile.clone());

    refresh_window_title(context);
    refresh_tile_styles(context);
    spawn_session(context, session_id, &session.launch, &tile.terminal);
}

fn build_tile(
    context: &Rc<AppContext>,
    session_id: SessionId,
    launch: &SessionLaunch,
    status: SessionStatus,
) -> SessionTileWidgets {
    let title = gtk::Label::builder()
        .label(&launch.name)
        .xalign(0.0)
        .css_classes(vec!["tile-title".to_string()])
        .build();
    let subtitle = gtk::Label::builder()
        .label(&launch.subtitle)
        .xalign(0.0)
        .css_classes(vec!["tile-subtitle".to_string()])
        .build();
    let status_label = build_status_label(status);

    let title_stack = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(2)
        .hexpand(true)
        .build();
    title_stack.append(&title);
    title_stack.append(&subtitle);

    let header = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(12)
        .margin_top(10)
        .margin_bottom(10)
        .margin_start(12)
        .margin_end(12)
        .build();
    header.append(&title_stack);
    header.append(&status_label);

    let terminal = vte::Terminal::builder()
        .scroll_on_output(true)
        .scroll_on_keystroke(true)
        .input_enabled(true)
        .hexpand(true)
        .vexpand(true)
        .build();
    terminal.set_scrollback_lines(10_000);
    terminal.add_css_class("terminal-surface");

    let terminal_wrapper = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .hexpand(true)
        .vexpand(true)
        .build();
    terminal_wrapper.append(&terminal);

    let shell_hint = gtk::Label::builder()
        .label("Native terminal stays primary")
        .xalign(0.0)
        .margin_top(8)
        .margin_bottom(10)
        .margin_start(12)
        .margin_end(12)
        .css_classes(vec!["tile-footnote".to_string()])
        .build();

    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .hexpand(true)
        .vexpand(true)
        .build();
    content.append(&header);
    content.append(&terminal_wrapper);
    content.append(&shell_hint);

    let root = gtk::Frame::builder()
        .hexpand(true)
        .vexpand(true)
        .child(&content)
        .build();
    root.add_css_class("session-tile");

    let click = gtk::GestureClick::new();
    {
        let context = context.clone();
        click.connect_pressed(move |_, _, _, _| {
            context.state.borrow_mut().select_session(session_id);
            refresh_tile_styles(&context);
        });
    }
    root.add_controller(click);

    let terminal_click = gtk::GestureClick::new();
    {
        let context = context.clone();
        let terminal = terminal.clone();
        terminal_click.connect_pressed(move |_, _, _, _| {
            {
                let mut state = context.state.borrow_mut();
                state.select_session(session_id);
                state.set_terminal_focus(Some(session_id));
            }
            terminal.grab_focus();
            refresh_tile_styles(&context);
        });
    }
    terminal.add_controller(terminal_click);

    {
        let context = context.clone();
        terminal.connect_notify_local(Some("has-focus"), move |term, _| {
            {
                let mut state = context.state.borrow_mut();
                if term.has_focus() {
                    state.select_session(session_id);
                    state.set_terminal_focus(Some(session_id));
                } else if state.focused_terminal() == Some(session_id) {
                    state.set_terminal_focus(None);
                }
            }
            refresh_tile_styles(&context);
        });
    }

    SessionTileWidgets {
        root,
        status: status_label,
        terminal,
    }
}

fn build_status_label(status: SessionStatus) -> gtk::Label {
    let label = gtk::Label::builder()
        .label(status.chip_label())
        .css_classes(vec![
            "status-chip".to_string(),
            status.css_class().to_string(),
        ])
        .build();
    label
}

fn spawn_session(
    context: &Rc<AppContext>,
    session_id: SessionId,
    launch: &SessionLaunch,
    terminal: &vte::Terminal,
) {
    let argv_owned = launch.argv();
    let argv: Vec<&str> = argv_owned.iter().map(String::as_str).collect();
    let envv: [&str; 0] = [];
    let cwd = launch.cwd.as_ref().and_then(|path| path.to_str());

    {
        let context = context.clone();
        terminal.connect_child_exited(move |_, exit_code| {
            context
                .state
                .borrow_mut()
                .mark_exited(session_id, exit_code);
            update_status_label(&context, session_id);
            refresh_window_title(&context);
            refresh_tile_styles(&context);
        });
    }

    {
        let context = context.clone();
        terminal.spawn_async(
            vte::PtyFlags::DEFAULT,
            cwd,
            &argv,
            &envv,
            glib::SpawnFlags::SEARCH_PATH,
            || {},
            -1,
            None::<&gio::Cancellable>,
            move |result| {
                match result {
                    Ok(pid) => {
                        context
                            .state
                            .borrow_mut()
                            .mark_spawned(session_id, pid.0 as u32);
                    }
                    Err(error) => {
                        eprintln!("failed to spawn session {session_id:?}: {error}");
                        context.state.borrow_mut().mark_exited(session_id, -1);
                    }
                }
                update_status_label(&context, session_id);
                refresh_window_title(&context);
                refresh_tile_styles(&context);
            },
        );
    }
}

fn update_status_label(context: &Rc<AppContext>, session_id: SessionId) {
    let Some(tile) = context.tiles.borrow().get(&session_id).cloned() else {
        return;
    };
    let status = {
        let state = context.state.borrow();
        let Some(session) = state
            .sessions()
            .iter()
            .find(|session| session.id == session_id)
        else {
            return;
        };
        session.status
    };

    tile.status.set_label(&status.chip_label());
    tile.status.remove_css_class("status-launching");
    tile.status.remove_css_class("status-live");
    tile.status.remove_css_class("status-exited-clean");
    tile.status.remove_css_class("status-exited-error");
    tile.status.add_css_class(status.css_class());
}

fn refresh_window_title(context: &Rc<AppContext>) {
    let state = context.state.borrow();
    let total = state.sessions().len();
    let live = state
        .sessions()
        .iter()
        .filter(|session| matches!(session.status, SessionStatus::Live))
        .count();
    let exited = state
        .sessions()
        .iter()
        .filter(|session| matches!(session.status, SessionStatus::Exited(_)))
        .count();
    context
        .title
        .set_subtitle(&format!("{total} sessions · {live} live · {exited} exited"));
}

fn refresh_tile_styles(context: &Rc<AppContext>) {
    let state = context.state.borrow();
    for (session_id, tile) in context.tiles.borrow().iter() {
        tile.root.remove_css_class("selected");
        tile.root.remove_css_class("terminal-focused");
        if state.selected_session() == Some(*session_id) {
            tile.root.add_css_class("selected");
        }
        if state.focused_terminal() == Some(*session_id) {
            tile.root.add_css_class("terminal-focused");
        }
    }
}

fn load_css() {
    let provider = gtk::CssProvider::new();
    provider.load_from_string(
        "
        window {
            background: linear-gradient(180deg, #0a111a 0%, #0f1622 100%);
        }

        .session-tile {
            border-radius: 18px;
            border: 1px solid rgba(160, 174, 192, 0.18);
            background: rgba(12, 19, 30, 0.96);
            box-shadow: 0 18px 36px rgba(0, 0, 0, 0.28);
        }

        .session-tile.selected {
            border-color: rgba(119, 198, 255, 0.85);
            box-shadow: 0 0 0 1px rgba(119, 198, 255, 0.9), 0 20px 44px rgba(11, 92, 161, 0.28);
        }

        .session-tile.terminal-focused {
            border-color: rgba(255, 189, 89, 0.85);
            box-shadow: 0 0 0 1px rgba(255, 189, 89, 0.95), 0 22px 46px rgba(255, 189, 89, 0.18);
        }

        .tile-title {
            font-weight: 700;
            font-size: 15px;
            color: #f8fafc;
        }

        .tile-subtitle {
            color: rgba(226, 232, 240, 0.72);
            font-size: 12px;
        }

        .tile-footnote {
            color: rgba(148, 163, 184, 0.82);
            font-size: 11px;
            letter-spacing: 0.04em;
        }

        .status-chip {
            border-radius: 999px;
            padding: 4px 10px;
            font-weight: 700;
            font-size: 11px;
        }

        .status-launching {
            background: rgba(96, 165, 250, 0.18);
            color: #93c5fd;
        }

        .status-live {
            background: rgba(74, 222, 128, 0.18);
            color: #86efac;
        }

        .status-exited-clean {
            background: rgba(226, 232, 240, 0.16);
            color: #cbd5e1;
        }

        .status-exited-error {
            background: rgba(248, 113, 113, 0.18);
            color: #fca5a5;
        }

        .pill {
            border-radius: 999px;
            padding: 6px 14px;
            background: rgba(119, 198, 255, 0.16);
            color: #dbeafe;
        }

        terminal {
            border-top: 1px solid rgba(148, 163, 184, 0.08);
            border-bottom-left-radius: 14px;
            border-bottom-right-radius: 14px;
            padding: 10px;
        }
        ",
    );

    gtk::style_context_add_provider_for_display(
        &gdk::Display::default().expect("display should exist during app startup"),
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}
