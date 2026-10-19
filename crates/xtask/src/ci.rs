//! `cargo xtask ci`: the full check a session runs on this Mac before marking a pull
//! request ready (SUITE-STANDARDS.md section 6, "CI, ArtCraft-style"), the same steps as
//! the weekly build's Linux job less the performance budgets. Stops at the first failure;
//! on success prints one line to paste under "Tested locally".

use std::path::Path;
use std::process::Command;

/// Each step: what it checks, then the command.
pub fn steps(cargo: &str) -> Vec<(&'static str, Vec<String>)> {
    let v = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    vec![
        ("format", v(&[cargo, "fmt", "--all", "--", "--check"])),
        (
            "clippy",
            v(&[
                cargo,
                "clippy",
                "--workspace",
                "--all-targets",
                "--locked",
                "--",
                "-D",
                "warnings",
            ]),
        ),
        ("tests", v(&[cargo, "test", "--workspace", "--locked"])),
        (
            "page tests",
            v(&[
                "node",
                "--test",
                "crates/method-schedule-app/page-tests/logic.test.mjs",
            ]),
        ),
        ("site", v(&["python3", "scripts/site.py", "--check"])),
        ("brand", v(&["python3", "scripts/make-brand.py", "--check"])),
    ]
}

pub fn command(root: &Path) -> Result<String, String> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let mut passed = Vec::new();
    for (what, args) in steps(&cargo) {
        eprintln!("xtask ci: {what}");
        let status = Command::new(&args[0])
            .args(&args[1..])
            .current_dir(root)
            .status()
            .map_err(|e| format!("couldn't run {}: {e}", args[0]))?;
        if !status.success() {
            return Err(format!("{what} failed"));
        }
        passed.push(what);
    }
    eprintln!("xtask ci: the WebAssembly check");
    crate::wasm::command(root)?;
    passed.push("wasm-check");
    Ok(format!("xtask ci: passed ({})\n", passed.join(", ")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_local_check_covers_the_weekly_linux_job() {
        let names: Vec<_> = steps("cargo").into_iter().map(|(w, _)| w).collect();
        assert_eq!(
            names,
            ["format", "clippy", "tests", "page tests", "site", "brand"]
        );
        let all: Vec<String> = steps("cargo").into_iter().flat_map(|(_, a)| a).collect();
        assert!(all.iter().any(|a| a == "--all-targets"));
        assert!(all.iter().any(|a| a == "warnings"));
    }
}
