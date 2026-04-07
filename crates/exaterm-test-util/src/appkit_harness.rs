//! Shared test harness for AppKit integration tests.

use std::sync::Once;

use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};
use objc2_foundation::{MainThreadMarker, NSDate, NSRunLoop};

static INIT_APP: Once = Once::new();

/// Ensures `NSApplication` is initialized exactly once with Accessory activation policy
/// (no Dock icon, but windows can display and the appearance system is active).
pub fn ensure_app(mtm: MainThreadMarker) {
    INIT_APP.call_once(|| {
        let app = NSApplication::sharedApplication(mtm);
        app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
    });
}

/// Spin the run loop briefly so pending AppKit events are processed.
pub fn flush_runloop() {
    let date = NSDate::dateWithTimeIntervalSinceNow(0.05);
    NSRunLoop::currentRunLoop().runUntilDate(&date);
}

/// Run a list of named test functions on the main thread.
/// Prints pass/fail for each and exits with appropriate code.
#[allow(clippy::print_stdout)]
pub fn run_tests(tests: &[(&str, fn(MainThreadMarker))]) {
    #[allow(clippy::expect_used)]
    let mtm = MainThreadMarker::new().expect("Custom harness must run on main thread");
    ensure_app(mtm);

    let mut passed: usize = 0;
    let mut failed: usize = 0;
    let total = tests.len();

    println!("\nrunning {} tests", total);

    for (name, test_fn) in tests {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            test_fn(mtm);
        }));

        match result {
            Ok(()) => {
                println!("test {} ... ok", name);
                passed = passed.saturating_add(1);
            }
            Err(e) => {
                let msg = if let Some(s) = e.downcast_ref::<&str>() {
                    (*s).to_string()
                } else if let Some(s) = e.downcast_ref::<String>() {
                    s.clone()
                } else {
                    "Box<dyn Any>".to_string()
                };
                println!("test {} ... FAILED\n  {}", name, msg);
                failed = failed.saturating_add(1);
            }
        }
    }

    println!(
        "\ntest result: {}. {} passed; {} failed; 0 ignored; 0 measured; 0 filtered out\n",
        if failed == 0 { "ok" } else { "FAILED" },
        passed,
        failed
    );

    if failed > 0 {
        std::process::exit(1);
    }
}
