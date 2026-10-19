//! `cargo xtask <command>`: Method Schedule's build tooling, as Method Budget's.
//!
//! ci           the full check before marking a pull request ready: format, clippy, the
//!              tests, the page tests, the site and brand checks, wasm-check
//! wasm-check   the crates a browser runs build for WebAssembly, and nothing in them
//!              pulls in a source of randomness (docs/research/web-session.md)

mod ci;
mod wasm;

use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "usage: cargo xtask <command>

  ci           the full check before marking a pull request ready
  wasm-check   the crates a browser runs build for WebAssembly
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
