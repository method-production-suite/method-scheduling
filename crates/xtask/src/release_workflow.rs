//! The release workflow runs no cargo beside a key (SUITE-STANDARDS.md section 3, made strict
//! 2026-10-19; Method Launcher's test, adapted): in each job, no step after the first one that
//! reads a secret may run `cargo`, since a stale fingerprint would rebuild and run build
//! scripts and proc macros beside the key. Those steps run prebuilt binaries instead
//! (`cargo-tauri`, installed checked by scripts/install-tauri-cli.sh).
//!
//! A unit test, not one in `tests/`: an integration test makes `cargo test` rebuild the xtask
//! binary, which Windows can't replace while `cargo xtask ci` is running it.

use std::path::Path;

/// The jobs of a workflow, each as its steps' text, from the YAML's indentation (jobs at two
/// spaces, steps at six).
fn jobs(workflow: &str) -> Vec<(String, Vec<String>)> {
    let mut jobs: Vec<(String, Vec<String>)> = Vec::new();
    let mut in_jobs = false;
    for line in workflow.lines() {
        if line == "jobs:" {
            in_jobs = true;
            continue;
        }
        if !in_jobs {
            continue;
        }
        let job = line
            .strip_prefix("  ")
            .and_then(|l| l.strip_suffix(':'))
            .filter(|n| !n.starts_with(' ') && !n.is_empty());
        if let Some(name) = job {
            jobs.push((name.to_string(), Vec::new()));
        } else if line.starts_with("      - ") {
            if let Some((_, steps)) = jobs.last_mut() {
                steps.push(format!("{line}\n"));
            }
        } else if line.starts_with("        ") {
            if let Some(step) = jobs.last_mut().and_then(|(_, steps)| steps.last_mut()) {
                step.push_str(line);
                step.push('\n');
            }
        }
    }
    jobs
}

/// Whether a step runs cargo, ignoring comments. `cargo-tauri`, the prebuilt binary, is a
/// word of its own and doesn't count.
fn runs_cargo(step: &str) -> bool {
    step.lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .any(|l| {
            l.split(|c: char| !c.is_alphanumeric() && c != '-')
                .any(|w| w == "cargo")
        })
}

fn violations(workflow: &str) -> Vec<String> {
    let mut found = Vec::new();
    for (job, steps) in jobs(workflow) {
        let Some(first) = steps.iter().position(|s| s.contains("secrets.")) else {
            continue;
        };
        for step in steps.iter().skip(first + 1).filter(|s| runs_cargo(s)) {
            found.push(format!(
                "{job}: {}",
                step.lines().next().unwrap_or_default().trim()
            ));
        }
    }
    found
}

#[test]
fn no_cargo_after_a_secret() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.github/workflows/release.yml");
    let workflow = std::fs::read_to_string(&path).expect("release.yml");
    let jobs = jobs(&workflow);
    assert!(
        jobs.iter()
            .any(|(name, steps)| name == "build" && steps.len() > 10),
        "the build job's steps were read"
    );
    assert_eq!(violations(&workflow), Vec::<String>::new());
}

/// SHA256SUMS is signed on every release, and the run stops if it can't be: no `if:` on the
/// steps, signed before the draft goes up, and the key proved before anything is built.
#[test]
fn the_release_signs_its_checksums() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.github/workflows/release.yml");
    let workflow = std::fs::read_to_string(&path).expect("release.yml");
    let jobs = jobs(&workflow);
    let steps = |job: &str| {
        &jobs
            .iter()
            .find(|(name, _)| name == job)
            .unwrap_or_else(|| panic!("no job {job}"))
            .1
    };
    let at = |job: &str, text: &str| {
        steps(job)
            .iter()
            .position(|s| s.contains(text))
            .unwrap_or_else(|| panic!("{job} has no step with {text}"))
    };
    let sums = at("draft", "sha256sum -- *");
    let sign = at("draft", "scripts/sign-file.sh dist/SHA256SUMS");
    let check = at("draft", "scripts/verify-file.sh dist/SHA256SUMS");
    let draft = at("draft", "gh release create");
    assert!(sums < sign && sign < check && check < draft);
    let probe = at("signing-probe", "scripts/sign-file.sh");
    let proved = at("signing-probe", "scripts/verify-file.sh");
    assert!(probe < proved);
    for s in [
        &steps("draft")[sign],
        &steps("draft")[check],
        &steps("signing-probe")[probe],
        &steps("signing-probe")[proved],
    ] {
        assert!(!s.contains("if:"), "a signing step can't be skipped: {s}");
    }
    // The jobs that hold the key take nothing unpinned from a package archive.
    for job in ["draft", "signing-probe"] {
        assert!(
            !steps(job).iter().any(|s| s.contains("apt-get")),
            "{job} installs from a package archive"
        );
        assert!(at(job, "scripts/install-minisign.sh") < at(job, "scripts/sign-file.sh"));
    }
    assert!(
        workflow.contains("needs: [check, signing-probe]"),
        "the build waits for the probe"
    );
    assert!(
        workflow.contains("needs: [check, build]"),
        "the draft follows the build"
    );
}

#[test]
fn the_check_catches_cargo_after_a_secret() {
    let bad = "jobs:\n  build:\n    steps:\n      - name: Compile\n        run: cargo tauri build --no-bundle\n      - name: Key\n        env:\n          K: ${{ secrets.KEY }}\n        run: echo\n      - name: Bundle\n        run: |\n          # cargo is fine in a comment\n          cargo tauri bundle\n";
    assert_eq!(violations(bad), vec!["build: - name: Bundle".to_string()]);
    let good = bad.replace("cargo tauri bundle", "cargo-tauri bundle");
    assert!(violations(&good).is_empty());
}
