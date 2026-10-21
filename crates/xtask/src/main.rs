//! `cargo xtask <command>`: Method Schedule's build tooling, as Method Budget's.
//!
//! ci           the full check before marking a pull request ready: format, clippy, the
//!              tests, the page tests, the site and brand checks, wasm-check
//! wasm-check   the crates a browser runs build for WebAssembly, and nothing in them
//!              pulls in a source of randomness (docs/research/web-session.md)
//! web          the page in a browser, ready to serve in <cargo's build folder>/web (`--smoke` then runs
//!              the headless smoke test on it)

mod ci;
#[cfg(test)]
mod release_workflow;
mod wasm;
mod web;

use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "usage: cargo xtask <command>

  ci           the full check before marking a pull request ready
  wasm-check   the crates a browser runs build for WebAssembly
  web          the page in a browser, in <build folder>/web [--out <dir>] [--smoke]
";

fn main() -> ExitCode {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the workspace root");
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("ci") => ci::command(&root),
        Some("wasm-check") => wasm::command(&root),
        Some("web") => web::command(&root, &args[1..]),
        _ => Err(USAGE.trim_end().to_string()),
    };
    match result {
        Ok(out) => {
            print!("{out}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("xtask: {e}");
            ExitCode::FAILURE
        }
    }
}
