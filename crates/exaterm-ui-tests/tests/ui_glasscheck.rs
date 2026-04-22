#[cfg(any(target_os = "linux", target_os = "macos"))]
mod imp {
    use exaterm_ui::ui_test_contract::{selectors, UiSessionKey, UiTestScenario};
    use glasscheck::{
        assert_contained_within_node, assert_count, assert_exists, assert_not_exists,
        LayoutTolerance, PollOptions, Selector,
    };

    #[cfg(target_os = "linux")]
    use exaterm_gtk::test_support::{mount_scenario, MountedGtkUi as MountedUi};
    #[cfg(target_os = "macos")]
    use exaterm_macos::test_support::{mount_scenario, mount_with_terminal, MountedAppKitUi as MountedUi};
    #[cfg(target_os = "macos")]
    use objc2_foundation::MainThreadMarker;

    pub fn main() {
        run("empty_workspace", empty_workspace);
        run("battlefield_single_sparse", battlefield_single_sparse);
        run(
            "battlefield_single_summarized",
            battlefield_single_summarized,
        );
        run("battlefield_four_mixed", battlefield_four_mixed);
        run("focus_single_summarized", focus_single_summarized);
        run("focus_single_sparse", focus_single_sparse);
        run("battlefield_click_enters_focus", battlefield_click_enters_focus);
        run("focus_exit", focus_exit);
        run("focus_switches_sessions", focus_switches_sessions);
        #[cfg(target_os = "macos")]
        run(
            "return_key_executes_embedded_terminal_command",
            return_key_executes_embedded_terminal_command,
        );
    }

    fn run(name: &str, test: impl FnOnce()) {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(test));
        match result {
            Ok(()) => println!("test {name} ... ok"),
            Err(error) => {
                if let Some(message) = error.downcast_ref::<String>() {
                    eprintln!("test {name} ... FAILED\n{message}");
                } else if let Some(message) = error.downcast_ref::<&str>() {
                    eprintln!("test {name} ... FAILED\n{message}");
                } else {
                    eprintln!("test {name} ... FAILED");
                }
                std::process::exit(1);
            }
        }
    }

    fn empty_workspace() {
        let (harness, mounted) = mounted(UiTestScenario::EmptyWorkspace);
        let scene = mounted.host.snapshot_scene();

        assert_exists(&scene, &selector(selectors::WORKSPACE_EMPTY_STATE)).unwrap();
        assert_not_exists(&scene, &selector(selectors::WORKSPACE_BATTLEFIELD)).unwrap();
        assert_not_exists(&scene, &selector(selectors::WORKSPACE_FOCUS_PANEL)).unwrap();
        assert_text(
            &scene,
            selectors::WORKSPACE_EMPTY_STATE_TITLE,
            "No Live Sessions Yet",
        );
        assert_text_contains(
            &scene,
            selectors::WORKSPACE_EMPTY_STATE_BODY,
            "Use Add Shell to start a real terminal-native agent",
        );
        harness.flush();
    }

    fn battlefield_single_sparse() {
        let (_harness, mounted) = mounted(UiTestScenario::BattlefieldSingleSparse);
        let scene = mounted.host.snapshot_scene();
        let card = selector(&selectors::battlefield_card(UiSessionKey::Shell1));

        assert_exists(&scene, &card).unwrap();
        assert_exists(
            &scene,
            &selector(&selectors::battlefield_card_terminal_slot(
                UiSessionKey::Shell1,
            )),
        )
        .unwrap();
        assert_not_exists(
            &scene,
            &selector(&selectors::battlefield_card_scrollback(
                UiSessionKey::Shell1,
            )),
        )
        .unwrap();
        assert_not_exists(
            &scene,
            &selector(&selectors::battlefield_card_title(UiSessionKey::Shell1)),
        )
        .unwrap();
        assert_not_exists(
            &scene,
            &selector(&selectors::battlefield_card_status(UiSessionKey::Shell1)),
        )
        .unwrap();
        assert_not_exists(
            &scene,
            &selector(&selectors::battlefield_card_headline(UiSessionKey::Shell1)),
        )
        .unwrap();
        assert_not_exists(
            &scene,
            &selector(&selectors::battlefield_card_nudge(UiSessionKey::Shell1)),
        )
        .unwrap();
        assert_not_exists(
            &scene,
            &selector(&selectors::battlefield_card_attention_bar(
                UiSessionKey::Shell1,
            )),
        )
        .unwrap();
        assert_contained_within_node(
            &scene,
            &card,
            &selector(selectors::WORKSPACE_BATTLEFIELD),
            LayoutTolerance::default(),
        )
        .unwrap();
    }

    fn battlefield_single_summarized() {
        let (_harness, mounted) = mounted(UiTestScenario::BattlefieldSingleSummarized);
        let scene = mounted.host.snapshot_scene();

        assert_exists(
            &scene,
            &selector(&selectors::battlefield_card(UiSessionKey::Shell1)),
        )
        .unwrap();
        assert_exists(
            &scene,
            &selector(&selectors::battlefield_card_title(UiSessionKey::Shell1)),
        )
        .unwrap();
        assert_exists(
            &scene,
            &selector(&selectors::battlefield_card_status(UiSessionKey::Shell1)),
        )
        .unwrap();
        assert_exists(
            &scene,
            &selector(&selectors::battlefield_card_headline(UiSessionKey::Shell1)),
        )
        .unwrap();
        assert_exists(
            &scene,
            &selector(&selectors::battlefield_card_nudge(UiSessionKey::Shell1)),
        )
        .unwrap();
        assert_exists(
            &scene,
            &selector(&selectors::battlefield_card_attention_bar(
                UiSessionKey::Shell1,
            )),
        )
        .unwrap();
        assert_exists(
            &scene,
            &selector(&selectors::battlefield_card_terminal_slot(
                UiSessionKey::Shell1,
            )),
        )
        .unwrap();
        assert_not_exists(&scene, &selector(selectors::WORKSPACE_EMPTY_STATE)).unwrap();
        assert_not_exists(&scene, &selector(selectors::WORKSPACE_FOCUS_PANEL)).unwrap();

        assert_text_contains(
            &scene,
            &selectors::battlefield_card_headline(UiSessionKey::Shell1),
            "Parser recovery narrowed to one failing transition",
        );
        assert_text_contains(
            &scene,
            &selectors::battlefield_card_nudge(UiSessionKey::Shell1),
            "AUTONUDGE",
        );
    }

    fn battlefield_four_mixed() {
        let (_harness, mounted) = mounted(UiTestScenario::BattlefieldFourMixed);
        let scene = mounted.host.snapshot_scene();
        let cards = [
            UiSessionKey::Shell1,
            UiSessionKey::Shell2,
            UiSessionKey::Shell3,
            UiSessionKey::Shell4,
        ]
        .map(|key| selector(&selectors::battlefield_card(key)));

        assert_count(
            &scene,
            &selector(&selectors::battlefield_card(UiSessionKey::Shell1)),
            1,
        )
        .unwrap();
        for card in &cards {
            assert_exists(&scene, card).unwrap();
        }
        let rects = cards
            .iter()
            .map(|card| scene.resolve(card).unwrap().bounds)
            .collect::<Vec<_>>();
        for (index, rect) in rects.iter().enumerate() {
            for other in rects.iter().skip(index + 1) {
                assert!(
                    !rects_overlap(*rect, *other),
                    "battlefield cards should not overlap"
                );
            }
        }

        for key in [
            UiSessionKey::Shell1,
            UiSessionKey::Shell2,
            UiSessionKey::Shell3,
            UiSessionKey::Shell4,
        ] {
            assert_exists(
                &scene,
                &selector(&selectors::battlefield_card_scrollback(key)),
            )
            .unwrap();
            assert_not_exists(
                &scene,
                &selector(&selectors::battlefield_card_terminal_slot(key)),
            )
            .unwrap();
        }
        assert_not_exists(
            &scene,
            &selector(&selectors::battlefield_card_title(UiSessionKey::Shell3)),
        )
        .unwrap();
        assert_not_exists(
            &scene,
            &selector(&selectors::battlefield_card_headline(UiSessionKey::Shell3)),
        )
        .unwrap();
        assert_not_exists(
            &scene,
            &selector(&selectors::battlefield_card_nudge(UiSessionKey::Shell3)),
        )
        .unwrap();
        assert_exists(
            &scene,
            &selector(&selectors::battlefield_card_title(UiSessionKey::Shell1)),
        )
        .unwrap();
        assert_exists(
            &scene,
            &selector(&selectors::battlefield_card_status(UiSessionKey::Shell2)),
        )
        .unwrap();
        assert_text(
            &scene,
            &selectors::battlefield_card_status(UiSessionKey::Shell2),
            "Blocked",
        );
        assert_text(
            &scene,
            &selectors::battlefield_card_status(UiSessionKey::Shell4),
            "STOPPED - 210s",
        );

        let selected_count = [
            UiSessionKey::Shell1,
            UiSessionKey::Shell2,
            UiSessionKey::Shell3,
            UiSessionKey::Shell4,
        ]
        .iter()
        .filter(|key| {
            scene
                .resolve(&selector(&selectors::battlefield_card(**key)))
                .unwrap()
                .node
                .state
                .get("selected")
                .is_some_and(|value| matches!(value, glasscheck::PropertyValue::Bool(true)))
        })
        .count();
        assert_eq!(
            selected_count, 1,
            "exactly one selected card should be visible"
        );
    }

    fn focus_single_summarized() {
        let (harness, mounted) = mounted(UiTestScenario::BattlefieldFourMixed);
        mounted
            .host
            .click_node(&battlefield_click_target(UiSessionKey::Shell2))
            .unwrap_or_else(|error| panic!("semantic click failed: {error}"));
        harness
            .wait_until(PollOptions::default(), || {
                mounted
                    .host
                    .snapshot_scene()
                    .count(&selector(selectors::WORKSPACE_FOCUS_PANEL))
                    > 0
            })
            .expect("focus panel should appear");

        let scene = mounted.host.snapshot_scene();
        assert_exists(&scene, &selector(selectors::WORKSPACE_FOCUS_PANEL)).unwrap();
        assert_exists(
            &scene,
            &selector(&selectors::focus_card(UiSessionKey::Shell2)),
        )
        .unwrap();
        assert_exists(
            &scene,
            &selector(&selectors::focus_card_title(UiSessionKey::Shell2)),
        )
        .unwrap();
        assert_exists(
            &scene,
            &selector(&selectors::focus_card_status(UiSessionKey::Shell2)),
        )
        .unwrap();
        assert_text(
            &scene,
            &selectors::focus_card_status(UiSessionKey::Shell2),
            "Blocked",
        );
        assert_exists(&scene, &selector(selectors::WORKSPACE_BATTLEFIELD)).unwrap();
        for key in [
            UiSessionKey::Shell1,
            UiSessionKey::Shell2,
            UiSessionKey::Shell3,
            UiSessionKey::Shell4,
        ] {
            assert_not_exists(
                &scene,
                &selector(&selectors::battlefield_card_terminal_slot(key)),
            )
            .unwrap();
        }
    }

    fn focus_single_sparse() {
        let (harness, mounted) = mounted(UiTestScenario::BattlefieldFourMixed);
        mounted
            .host
            .click_node(&battlefield_click_target(UiSessionKey::Shell3))
            .unwrap_or_else(|error| panic!("semantic click failed: {error}"));
        harness
            .wait_until(PollOptions::default(), || {
                mounted
                    .host
                    .snapshot_scene()
                    .count(&selector(&selectors::focus_card(UiSessionKey::Shell3)))
                    > 0
            })
            .expect("sparse card click should enter focus");

        let scene = mounted.host.snapshot_scene();
        assert_exists(
            &scene,
            &selector(&selectors::focus_card(UiSessionKey::Shell3)),
        )
        .unwrap();
        assert_not_exists(
            &scene,
            &selector(&selectors::focus_card_title(UiSessionKey::Shell3)),
        )
        .unwrap();
        assert_not_exists(
            &scene,
            &selector(&selectors::focus_card_status(UiSessionKey::Shell3)),
        )
        .unwrap();
        assert_not_exists(
            &scene,
            &selector(&selectors::focus_card_headline(UiSessionKey::Shell3)),
        )
        .unwrap();
        assert_not_exists(
            &scene,
            &selector(&selectors::focus_card_attention_pill(UiSessionKey::Shell3)),
        )
        .unwrap();
    }

    fn battlefield_click_enters_focus() {
        let (harness, mounted) = mounted(UiTestScenario::BattlefieldFourMixed);
        mounted
            .host
            .click_node(&battlefield_click_target(UiSessionKey::Shell1))
            .unwrap_or_else(|error| panic!("semantic click failed: {error}"));
        harness
            .wait_until(PollOptions::default(), || {
                mounted
                    .host
                    .snapshot_scene()
                    .count(&selector(&selectors::focus_card(UiSessionKey::Shell1)))
                    > 0
            })
            .expect("clicking a non-embedded battlefield card should enter focus");

        let scene = mounted.host.snapshot_scene();
        assert_exists(
            &scene,
            &selector(&selectors::focus_card(UiSessionKey::Shell1)),
        )
        .unwrap();
        assert_not_exists(
            &scene,
            &selector(&selectors::focus_card(UiSessionKey::Shell2)),
        )
        .unwrap();
    }

    fn focus_exit() {
        let (harness, mounted) = mounted(UiTestScenario::BattlefieldFourMixed);
        mounted
            .host
            .click_node(&battlefield_click_target(UiSessionKey::Shell2))
            .unwrap_or_else(|error| panic!("semantic click failed: {error}"));
        harness
            .wait_until(PollOptions::default(), || {
                mounted
                    .host
                    .snapshot_scene()
                    .count(&selector(selectors::WORKSPACE_FOCUS_PANEL))
                    > 0
            })
            .expect("focus panel should appear");

        mounted
            .host
            .click_node(&battlefield_click_target(UiSessionKey::Shell2))
            .unwrap_or_else(|error| panic!("semantic click failed: {error}"));
        harness
            .wait_until(PollOptions::default(), || {
                mounted
                    .host
                    .snapshot_scene()
                    .count(&selector(selectors::WORKSPACE_FOCUS_PANEL))
                    == 0
            })
            .expect("focus panel should close");

        let scene = mounted.host.snapshot_scene();
        assert_not_exists(&scene, &selector(selectors::WORKSPACE_FOCUS_PANEL)).unwrap();
        for key in [
            UiSessionKey::Shell1,
            UiSessionKey::Shell2,
            UiSessionKey::Shell3,
            UiSessionKey::Shell4,
        ] {
            assert_exists(&scene, &selector(&selectors::battlefield_card(key))).unwrap();
        }
    }

    fn focus_switches_sessions() {
        let (harness, mounted) = mounted(UiTestScenario::BattlefieldFourMixed);
        mounted
            .host
            .click_node(&battlefield_click_target(UiSessionKey::Shell2))
            .unwrap_or_else(|error| panic!("semantic click failed: {error}"));
        harness
            .wait_until(PollOptions::default(), || {
                mounted
                    .host
                    .snapshot_scene()
                    .count(&selector(selectors::WORKSPACE_FOCUS_PANEL))
                    > 0
            })
            .expect("focus panel should appear");

        mounted
            .host
            .click_node(&battlefield_click_target(UiSessionKey::Shell4))
            .unwrap_or_else(|error| panic!("semantic click failed: {error}"));
        harness
            .wait_until(PollOptions::default(), || {
                mounted
                    .host
                    .snapshot_scene()
                    .count(&selector(&selectors::focus_card(UiSessionKey::Shell4)))
                    > 0
            })
            .expect("focus panel should switch sessions");

        let scene = mounted.host.snapshot_scene();
        assert_exists(
            &scene,
            &selector(&selectors::focus_card(UiSessionKey::Shell4)),
        )
        .unwrap();
        assert_not_exists(
            &scene,
            &selector(&selectors::focus_card(UiSessionKey::Shell2)),
        )
        .unwrap();
    }

    #[cfg(target_os = "macos")]
    fn return_key_executes_embedded_terminal_command() {
        use objc2_app_kit::NSEventModifierFlags;

        let mtm = MainThreadMarker::new().expect("main thread");
        let harness = glasscheck::Harness::new(mtm);
        let mounted = mount_with_terminal(&harness)
            .expect("mount_with_terminal should succeed");

        // Send Return (key code 36) through the queued path so it flows through the
        // AppKit local event monitor before reaching SwiftTerm as first responder.
        //
        // If the bug is present (monitor consumes key 36), SwiftTerm never receives
        // the event, the input handler is never called, and received_bytes stays empty.
        // With the fix (monitor passes key 36 through), SwiftTerm calls the input handler
        // with the Return bytes.
        mounted
            .host
            .input()
            .key_press_raw_queued(36, NSEventModifierFlags::empty(), "\r")
            .expect("key_press_raw_queued should succeed");

        harness.settle(2);

        let bytes = mounted.received_bytes.borrow();
        assert!(
            bytes.contains(&b'\r') || bytes.contains(&b'\n'),
            "Return key (code 36) should have been dispatched to the embedded terminal \
             input handler, but no carriage-return or newline was received.\n\
             Received bytes: {bytes:?}\n\
             This test fails when the event monitor consumes key code 36 instead of \
             passing it through to SwiftTerm."
        );
    }

    fn mounted(scenario: UiTestScenario) -> (glasscheck::Harness, MountedUi) {
        #[cfg(target_os = "linux")]
        let harness = glasscheck::Harness::new().expect("GTK should initialize");
        #[cfg(target_os = "macos")]
        let harness = {
            let mtm = MainThreadMarker::new().expect("AppKit tests must run on the main thread");
            glasscheck::Harness::new(mtm)
        };
        let mounted = mount_scenario(&harness, scenario).expect("scenario should mount");
        (harness, mounted)
    }

    fn selector(value: &str) -> Selector {
        Selector::selector_eq(value)
    }

    fn battlefield_click_target(key: UiSessionKey) -> Selector {
        Selector::id_eq(selectors::battlefield_card(key))
    }

    fn assert_text(scene: &glasscheck::Scene, selector_name: &str, text: &str) {
        let resolved = scene.resolve(&selector(selector_name)).unwrap();
        assert_eq!(resolved.node.label.as_deref(), Some(text));
    }

    fn assert_text_contains(scene: &glasscheck::Scene, selector_name: &str, text: &str) {
        let resolved = scene.resolve(&selector(selector_name)).unwrap();
        let label = resolved.node.label.clone().unwrap_or_default();
        assert!(
            label.contains(text),
            "expected {selector_name} label to contain {text:?}, got {label:?}"
        );
    }

    fn rects_overlap(a: glasscheck::Rect, b: glasscheck::Rect) -> bool {
        !(a.origin.x + a.size.width <= b.origin.x
            || b.origin.x + b.size.width <= a.origin.x
            || a.origin.y + a.size.height <= b.origin.y
            || b.origin.y + b.size.height <= a.origin.y)
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn main() {
    imp::main();
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn main() {}
