# Contributing to Method Schedule

Reports from people who schedule films are as welcome as patches. Questions and ideas
go to [Discussions](https://github.com/method-production-suite/method-scheduling/discussions).

## Reporting a problem

In the app, Help > Report a Bug opens a prefilled issue. Otherwise,
[open an issue](https://github.com/method-production-suite/method-scheduling/issues/new/choose)
and say which computer (Mac, Windows or Linux) and which part: the app or the
`method-schedule` command line.

The most useful reports:

- **A count that looks wrong.** Say which element, what the day out of days shows and what
  you expected. Explain This Count (Cmd+I on the row, or
  `method-schedule explain FILE ELEMENT`) prints the reasoning date by date; pasting it
  shows exactly where the two part.
- **A board or report layout your productions use**, described in words or drawn with
  invented content.
- **A script that imports badly**: what it was written in, what came out wrong and what
  you expected.

**Never attach or quote a real schedule or script.** An invented one that shows the
problem is always enough: `method-schedule sample --demo FILE` writes a small invented
production to start from, and an invented sample schedule is itself a welcome
contribution.

## Getting set up

Method Schedule is one Cargo workspace. You need Rust from [rustup](https://rustup.rs);
the toolchain is pinned in `rust-toolchain.toml`. There is no JavaScript toolchain.

```bash
cargo test --workspace                                 # every crate's tests
cargo fmt --all -- --check                             # formatting
cargo clippy --workspace --all-targets -- -D warnings  # lints, warnings as errors
cargo run -p method-schedule-cli -- help               # the command line
cargo run -p method-schedule-app                       # the window
```

Linux needs the WebKitGTK development packages to build the window; the list is in
[docs/APP.md](docs/APP.md#run-and-build).

**Before you open a pull request, run the full check on your own computer:**

```bash
cargo xtask ci
```

It runs formatting, clippy, every test, the page's tests (Node, which comes with most
systems), the site and brand checks and the WebAssembly check, and ends with
`xtask ci: passed (...)`. Paste that line into the pull request. GitHub runs only the two
quick dependency checks on a pull request; the full build runs weekly on macOS, Windows
and Linux, and again for every release. The app guide also covers the architecture and
how the window is tested.

## Making a change

- Start from an issue labelled
  [good first issue](https://github.com/method-production-suite/method-scheduling/labels/good%20first%20issue),
  or say on an issue what you plan before a large change.
- Read [CLEAN-ROOM.md](CLEAN-ROOM.md) first. Method Schedule is an independent,
  clean-room project: describe behavior in your own words, and never copy another
  product's code, help text, layouts, templates or sample data.
- Keep every test passing and add one for what you change. The file format
  ([docs/FORMAT.md](docs/FORMAT.md)) and the budget exchange file
  ([docs/EXCHANGE.md](docs/EXCHANGE.md)) are public specs: change them only additively.
- Changes made with AI coding tools are welcome on an accepted issue, with passing tests,
  from an author who can explain every line. Say in the pull request if you used them.
- Feature list, README and website copy: edit `docs/features.toml` and run
  `python3 scripts/site.py` ([docs/WEBSITE.md](docs/WEBSITE.md)).

By contributing you agree that your work is licensed under MIT OR Apache-2.0, as the
project is.
