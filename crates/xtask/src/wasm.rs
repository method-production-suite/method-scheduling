//! `cargo xtask wasm-check`: the crates a browser runs build for WebAssembly
//! (docs/research/web-session.md, decision 222). The session crate is checked with
//! its default features only, never `std-clock`, so nothing that only works on a
//! desktop sneaks in; and no `getrandom` may be under it, since nothing it does needs
//! randomness and the browser target has none without a host's help.

use std::path::Path;
use std::process::Command;

/// The target a browser build uses.
pub const TARGET: &str = "wasm32-unknown-unknown";

/// The crates checked.
pub const CRATES: &[&str] = &[
    "method-schedule-core",
    "method-schedule-format",
    "method-schedule-session",
];

/// The `cargo check` arguments for the crates in [`CRATES`].
pub fn check_args() -> Vec<String> {
    let mut args = vec!["check".to_string(), "--locked".to_string()];
    for c in CRATES {
        args.push("-p".into());
        args.push((*c).into());
    }
    args.push("--target".into());
    args.push(TARGET.into());
    args
}

/// The `cargo tree` arguments that list what pulls `getrandom` into the session crate
/// on [`TARGET`]; any line of output is a failure.
pub fn tree_args() -> Vec<String> {
    [
        "tree",
        "--locked",
        "-p",
        "method-schedule-session",
        "--target",
        TARGET,
        "-e",
        "normal",
        "-i",
        "getrandom",
    ]
    .map(String::from)
    .to_vec()
}

/// Whether rustup has [`TARGET`].
pub fn target_installed(root: &Path) -> bool {
    Command::new("rustup")
        .args(["target", "list", "--installed"])
        .current_dir(root)
        .output()
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .any(|l| l.trim() == TARGET)
        })
        .unwrap_or(false)
}

pub fn command(root: &Path) -> Result<String, String> {
    if !target_installed(root) {
        return Err(format!(
            "the {TARGET} target isn't installed; run `rustup target add {TARGET}` first"
        ));
    }
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let status = Command::new(&cargo)
        .args(check_args())
        .current_dir(root)
        .status()
        .map_err(|e| format!("couldn't run cargo: {e}"))?;
    if !status.success() {
        return Err(format!("{} don't build for {TARGET}", CRATES.join(", ")));
    }
    // cargo tree -i fails when the crate isn't in the graph at all, which is the
    // good case; what matters is whether it printed a tree.
    let out = Command::new(&cargo)
        .args(tree_args())
        .current_dir(root)
        .output()
        .map_err(|e| format!("couldn't run cargo: {e}"))?;
    let tree = String::from_utf8_lossy(&out.stdout);
    if out.status.success() && !tree.trim().is_empty() {
        return Err(format!(
            "getrandom is under method-schedule-session on {TARGET}:\n{tree}"
        ));
    }
    Ok(format!(
        "{} build for {TARGET}, with no getrandom under the session\n",
        CRATES.join(", ")
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_session_crate_is_checked_for_the_browser_without_features() {
        let args = check_args();
        assert!(args
            .windows(2)
            .any(|w| w == ["-p", "method-schedule-session"]));
        assert!(args.windows(2).any(|w| w == ["--target", TARGET]));
        assert!(!args.iter().any(|a| a.contains("feature")));
    }

    #[test]
    fn the_randomness_check_looks_at_the_session_on_the_browser_target() {
        let args = tree_args();
        assert!(args.windows(2).any(|w| w == ["-i", "getrandom"]));
        assert!(args.windows(2).any(|w| w == ["--target", TARGET]));
    }
}
