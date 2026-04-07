pub mod pixel_compare;

#[cfg(feature = "appkit")]
pub mod appkit_harness;

#[cfg(feature = "appkit")]
pub mod capture;

use std::path::PathBuf;

/// Create a temporary directory inside the workspace `target/` directory.
///
/// This avoids relying on the system temp directory, which may be
/// inaccessible from within sandboxed environments. The returned `TempDir`
/// is automatically cleaned up when dropped.
///
/// # Panics
///
/// Panics if the workspace `target/` directory cannot be determined or
/// if tempdir creation fails.
#[must_use]
#[allow(clippy::expect_used)]
pub fn test_tempdir() -> tempfile::TempDir {
    let target_dir = workspace_target_dir();
    tempfile::tempdir_in(target_dir).expect("failed to create tempdir in workspace target/")
}

#[allow(clippy::expect_used)]
fn workspace_target_dir() -> PathBuf {
    // CARGO_MANIFEST_DIR points to crates/exaterm-test-util;
    // go up two levels to reach the workspace root, then into target/.
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("workspace root should exist two levels above CARGO_MANIFEST_DIR")
        .join("target")
}
