//! `cargo xtask web [--out <dir>] [--smoke]`: the page in a browser
//! (docs/research/web-session.md, phase 2). Builds `method-schedule-web` for
//! wasm32-unknown-unknown, runs the `wasm-bindgen` command line on it and puts a folder
//! ready to serve in `target/web/`:
//!
//! ```text
//! target/web/
//!   index.html, *.css, js/                      the page (crates/method-schedule-app/ui),
//!                                               index.html loading the shim first
//!   method-schedule-web/tauri-shim.js           the page's `invoke` and events over the Host
//!   method-schedule-web/method_schedule_web.js  wasm-bindgen's JS (an ES module)
//!   method-schedule-web/method_schedule_web_bg.wasm
//! ```
//!
//! With `--smoke`, then runs `scripts/web-smoke.mjs` under Node on the built module: the
//! sample opens, a command edits it, a report exports, the schedule saves and opens again.
//! Headless: nothing is shown.
//!
//! No npm and no bundler. The `wasm-bindgen` command line must be the version in
//! Cargo.lock: `cargo install wasm-bindgen-cli --version <it> --locked`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::wasm::{target_installed, TARGET};

/// The cargo profile the browser build uses (the root Cargo.toml).
pub const PROFILE: &str = "web-release";

const CRATE: &str = "method-schedule-web";
const SHIM: &str = "crates/method-schedule-web/js/tauri-shim.js";
const UI: &str = "crates/method-schedule-app/ui";
const SMOKE: &str = "scripts/web-smoke.mjs";
const SUBDIR: &str = "method-schedule-web";

/// The page's entry script, before which the shim is loaded.
const MAIN: &str = r#"<script type="module" src="js/main.js"></script>"#;
const SHIM_TAG: &str = r#"<script src="method-schedule-web/tauri-shim.js"></script>"#;

/// The version of crate `name` in Cargo.lock's text.
pub fn locked_version(lock: &str, name: &str) -> Option<String> {
    let header = format!("name = \"{name}\"\n");
    let at = lock.find(&header)? + header.len();
    let line = lock[at..].lines().next()?;
    line.strip_prefix("version = \"")?
        .strip_suffix('"')
        .map(str::to_string)
}

/// The page's index.html with the shim loaded before its modules.
pub fn patch_index(html: &str) -> Result<String, String> {
    if html.contains(SHIM_TAG) {
        return Ok(html.to_string());
    }
    if html.matches(MAIN).count() != 1 {
        return Err(format!(
            "{UI}/index.html doesn't load js/main.js once as expected"
        ));
    }
    Ok(html.replace(MAIN, &format!("{SHIM_TAG}\n    {MAIN}")))
}

/// The `cargo build` arguments for the browser build.
pub fn build_args() -> Vec<String> {
    [
        "build",
        "--locked",
        "-p",
        CRATE,
        "--target",
        TARGET,
        "--profile",
        PROFILE,
    ]
    .map(String::from)
    .to_vec()
}

pub fn command(root: &Path, args: &[String]) -> Result<String, String> {
    let mut out = None;
    let mut smoke = false;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--out" => out = Some(PathBuf::from(it.next().ok_or("--out needs a folder")?)),
            "--smoke" => smoke = true,
            other => return Err(format!("xtask web doesn't take {other}")),
        }
    }
    if !target_installed(root) {
        return Err(format!(
            "the {TARGET} target isn't installed; run `rustup target add {TARGET}` first"
        ));
    }
    let lock = fs::read_to_string(root.join("Cargo.lock"))
        .map_err(|e| format!("couldn't read Cargo.lock: {e}"))?;
    let want = locked_version(&lock, "wasm-bindgen").ok_or("Cargo.lock has no wasm-bindgen")?;
    let install = format!("cargo install wasm-bindgen-cli --version {want} --locked");
    let have = Command::new("wasm-bindgen")
        .arg("--version")
        .output()
        .map_err(|_| format!("the wasm-bindgen command line isn't installed; run `{install}`"))?;
    let have = String::from_utf8_lossy(&have.stdout).trim().to_string();
    if have != format!("wasm-bindgen {want}") {
        return Err(format!(
            "wasm-bindgen is `{have}` but Cargo.lock has {want}; run `{install}`"
        ));
    }

    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let status = Command::new(cargo)
        .args(build_args())
        .current_dir(root)
        .status()
        .map_err(|e| format!("couldn't run cargo: {e}"))?;
    if !status.success() {
        return Err(format!("{CRATE} doesn't build for {TARGET}"));
    }
    let wasm = root
        .join("target")
        .join(TARGET)
        .join(PROFILE)
        .join("method_schedule_web.wasm");

    let out = out.unwrap_or_else(|| root.join("target").join("web"));
    clear(&out)?;
    copy_dir(&root.join(UI), &out)?;
    let index = out.join("index.html");
    let html = fs::read_to_string(&index).map_err(|e| format!("couldn't read index.html: {e}"))?;
    fs::write(&index, patch_index(&html)?)
        .map_err(|e| format!("couldn't write index.html: {e}"))?;

    let dir = out.join(SUBDIR);
    let status = Command::new("wasm-bindgen")
        .args([
            "--target",
            "web",
            "--no-typescript",
            "--remove-name-section",
            "--remove-producers-section",
            "--out-dir",
        ])
        .arg(&dir)
        .arg(&wasm)
        .status()
        .map_err(|e| format!("couldn't run wasm-bindgen: {e}"))?;
    if !status.success() {
        return Err("wasm-bindgen failed".into());
    }
    fs::copy(root.join(SHIM), dir.join("tauri-shim.js"))
        .map_err(|e| format!("couldn't copy the shim: {e}"))?;

    let size = fs::metadata(dir.join("method_schedule_web_bg.wasm"))
        .map(|m| m.len())
        .unwrap_or(0);
    let mut said = format!(
        "the page and {CRATE} are in {} (method_schedule_web_bg.wasm: {:.1} MB)\n\
         serve it with: python3 -m http.server -d {} 8000\n",
        out.display(),
        size as f64 / 1_000_000.0,
        out.display()
    );
    if smoke {
        said.push_str(&run_smoke(root, &dir)?);
    }
    Ok(said)
}

/// Runs the headless smoke test on the built module in `dir`, and checks the shim parses.
fn run_smoke(root: &Path, dir: &Path) -> Result<String, String> {
    let node = |args: &[&std::ffi::OsStr]| {
        Command::new("node")
            .args(args)
            .current_dir(root)
            .output()
            .map_err(|_| "Node isn't installed; the smoke test runs under it".to_string())
    };
    let check = node(&["--check".as_ref(), root.join(SHIM).as_os_str()])?;
    if !check.status.success() {
        return Err(format!(
            "the shim doesn't parse:\n{}",
            String::from_utf8_lossy(&check.stderr)
        ));
    }
    let run = node(&[root.join(SMOKE).as_os_str(), dir.as_os_str()])?;
    let text = String::from_utf8_lossy(&run.stdout).into_owned();
    if !run.status.success() {
        return Err(format!(
            "the smoke test failed:\n{text}{}",
            String::from_utf8_lossy(&run.stderr)
        ));
    }
    Ok(text)
}

/// Empties `out` for a fresh build: only a folder this command made (it holds the shim),
/// or one that doesn't exist or is empty.
fn clear(out: &Path) -> Result<(), String> {
    if !out.exists() {
        return Ok(());
    }
    let ours = out.join(SUBDIR).join("tauri-shim.js").is_file();
    let empty = fs::read_dir(out)
        .map(|mut d| d.next().is_none())
        .unwrap_or(false);
    if !ours && !empty {
        return Err(format!(
            "{} isn't empty and isn't an earlier web build; pick another --out",
            out.display()
        ));
    }
    fs::remove_dir_all(out).map_err(|e| format!("couldn't clear {}: {e}", out.display()))
}

fn copy_dir(from: &Path, to: &Path) -> Result<(), String> {
    fs::create_dir_all(to).map_err(|e| format!("couldn't make {}: {e}", to.display()))?;
    let entries =
        fs::read_dir(from).map_err(|e| format!("couldn't read {}: {e}", from.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name();
        if name.to_string_lossy().starts_with('.') {
            continue;
        }
        let path = entry.path();
        if path.is_dir() {
            copy_dir(&path, &to.join(&name))?;
        } else {
            fs::copy(&path, to.join(&name))
                .map_err(|e| format!("couldn't copy {}: {e}", path.display()))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cli_version_comes_from_the_lock_file() {
        let lock =
            "[[package]]\nname = \"wasm-bindgen\"\nversion = \"0.2.128\"\nsource = \"x\"\n\n\
                    [[package]]\nname = \"wasm-bindgen-macro\"\nversion = \"0.2.128\"\n";
        assert_eq!(
            locked_version(lock, "wasm-bindgen").as_deref(),
            Some("0.2.128")
        );
        assert_eq!(locked_version(lock, "js-sys"), None);
        let real = include_str!("../../../Cargo.lock");
        assert!(locked_version(real, "wasm-bindgen").is_some());
    }

    #[test]
    fn the_shim_loads_before_the_page() {
        let html = include_str!("../../method-schedule-app/ui/index.html");
        let out = patch_index(html).unwrap();
        let shim = out.find(SHIM_TAG).unwrap();
        assert!(shim < out.find(MAIN).unwrap());
        assert_eq!(patch_index(&out).unwrap(), out);
        assert!(patch_index("<html></html>").is_err());
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        assert!(root.join(SHIM).is_file());
        assert!(root.join(SMOKE).is_file());
    }

    #[test]
    fn the_browser_build_uses_its_own_profile() {
        let args = build_args();
        assert!(args.windows(2).any(|w| w == ["-p", CRATE]));
        assert!(args.windows(2).any(|w| w == ["--target", TARGET]));
        assert!(args.windows(2).any(|w| w == ["--profile", PROFILE]));
        let root = include_str!("../../../Cargo.toml");
        assert!(root.contains(&format!("[profile.{PROFILE}]")));
    }

    /// Every command the shim hands a picked file to is one the Host takes a file for,
    /// and the shim's quiet answers are for commands the Host doesn't run.
    #[test]
    fn the_shim_matches_the_host() {
        let shim = include_str!("../../method-schedule-web/js/tauri-shim.js");
        let picks = shim
            .split("const PICKS = {")
            .nth(1)
            .and_then(|t| t.split("};").next())
            .unwrap();
        let commands = method_schedule_session::host::commands();
        for line in picks.lines() {
            let Some((name, _)) = line.trim().split_once(':') else {
                continue;
            };
            assert!(commands.contains(&name), "the Host has no {name}");
        }
        let quiet = shim
            .split("const QUIET = {")
            .nth(1)
            .and_then(|t| t.split("\n  };").next())
            .unwrap();
        for line in quiet.lines() {
            let l = line.trim();
            if l.starts_with("//") {
                continue;
            }
            if let Some((name, _)) = l.split_once(':') {
                assert!(!commands.contains(&name), "{name} is the Host's, not quiet");
            }
        }
    }
}
