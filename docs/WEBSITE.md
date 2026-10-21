# The website and the feature list

How Method Schedule's public face stays in step with the code: one feature file feeds the
README and the website, release notes feed the site's "What's new" box and its release
notes page, a script regenerates all of it, and CI fails when they drift. How the site
looks (name, voice, colors, type, the mark) is in [BRAND.md](BRAND.md).

The system is Method Budget's (`../method-budget/docs/WEBSITE.md`), copied and adapted,
so the two sites are built, checked and published the same way and read as one family.

```
docs/features.toml ───┐                       ┌─> README.md                (feature section, clone command)
docs/parity/          ├─> scripts/site.py ────┼─> site/index.html          (feature list, What's new, repo links)
  features.csv ───────┤   (--check in CI)     └─> site/releases/index.html (every release note, repo links)
docs/releases/*.md ───┘                                │
                     the suite website repo copies site/ from the latest published release tag (below)
                                                       │
                                                methodschedule.com
```

**The repository is private for now.** Every source, issue, discussion and file-format
link on the site points at `https://github.com/method-production-suite/method-scheduling`; they work
once the repository is public, and until then only for people with access.

## The feature file: docs/features.toml

Every feature the README and the site list is one `[[feature]]` entry: a `group`, a short
`title`, one sentence of `text`, a `status` (`have` built, `soon` planned) and `ref`, the
parity rows and decisions behind it. Groups appear in the order the `[[group]]` tables
are written. The header of the file has the full field list and the writing rules
(user-facing, no em dashes, no vendor names).

`scripts/site.py` validates it before writing anything. The rule that matters most: a
`have` feature needs at least one ref, and every parity row it names (`BRK-07`, the `id`
column of docs/parity/features.csv) must have the status `have`; otherwise it is an
error, so the public list can never claim something that is not built. Decision refs
(`D44`) must name an entry in docs/decisions.md. A `soon` feature whose parity rows are
all `have` prints a warning, as a hint to flip it.

`[project] repo` at the top is the source repository's URL. It is written into every site
link marked `data-repo` and into the README's clone command, so moving the repository is
one line there plus a regenerate.

## The script: scripts/site.py

Python 3.11 or later, standard library only (it uses `tomllib`).

```bash
python3 scripts/site.py           # rewrite the generated blocks
python3 scripts/site.py --check   # change nothing; exit 1 if anything is out of date
python3 scripts/site.py --list    # print the features with their status and refs
python3 scripts/site.py --files   # print the files it writes
```

It only touches text between marker comments, so everything else in the generated files
is edited by hand as usual:

| File | Block | From |
|---|---|---|
| README.md | `<!-- features:start -->` to `<!-- features:end -->` | docs/features.toml |
| README.md | `<!-- clone:start -->` to `<!-- clone:end -->` | `[project] repo` |
| site/index.html | `<!-- features:start -->` to `<!-- features:end -->` | docs/features.toml |
| site/index.html | `<!-- whatsnew:start -->` to `<!-- whatsnew:end -->` | the newest docs/releases/<version>.md |
| site/index.html | `href` of every `<a href="..." data-repo="path">` | `[project] repo` |
| site/releases/index.html | `<!-- releases:start -->` to `<!-- releases:end -->` | every docs/releases/<version>.md |
| site/releases/index.html | `href` of every `data-repo` link | `[project] repo` |

A `data-repo` path becomes a `blob/main/<path>` link (`data-repo=""` is the repository
itself). Links to issues, discussions and `releases/latest` are not files, so they are
written as plain `href`s without `data-repo`, and need a hand edit if the repository
moves.

"What's new" takes the newest release note by version number (a prerelease sorts before
its release): its `# ` title, its first paragraph and up to six bullets, with bold kept
and links reduced to their text. The release notes page renders every note in full,
newest first, each with an anchor (`releases/#v0-1-0`). A picture in a note is a line of
its own, `![alt](../../assets/screens/board-light.png)`; the page draws it from `site/img`
by file name (the `-dark` capture in a dark scheme), and the script stops if that file
isn't there. With no notes at all, both say "No release yet".

Exit codes: 0 done or up to date, 1 out of date (`--check`), 2 the input is wrong (the
message names the entry and the problem; nothing was written).

CI runs `python3 scripts/site.py --check` and `python3 scripts/make-brand.py --check` in
the `site` job of `.github/workflows/ci.yml`, and the Pages workflow runs both again
before deploying.

## The site: site/

Static files, no build step, no JavaScript, no trackers, no cookies. Every page carries
a Content-Security-Policy that allows only this site's own styles and images
(`default-src 'none'; style-src 'self'; img-src 'self'; base-uri 'none'; form-action
'none'`), so no inline style, script or third-party request can slip in unnoticed. System
fonts only.

| Path | What it is |
|---|---|
| `index.html` | The home page: hero with the wordmark and the Early alpha pill, the board of Field Station (a picture of the real app, "Pictures" below), A look around (the other three pictures), What's new, Yours to keep (why open files), Explain this count (real output from the invented sample), Bring your schedules with you (scripts in, open formats out, the budget exchange file), Features, Keyboard first, Command line and coding agents, The day out of days feeds the budget, Download (no release yet, build from source, and the platform status table), Help build it (a door for film knowledge and one for code), FAQ, and a footer linking the Method Production Suite |
| `releases/index.html` | Every release note (generated block inside a hand-written page); empty for now |
| `404.html` | Served for any address with no page. Its links start at the site root, so it is right on methodschedule.com |
| `assets/tokens.css` | Brand colors and type, light and dark ([BRAND.md](BRAND.md)); Method Budget's token names and neutrals with our accent |
| `assets/site.css` | Layout and components; names no colors of its own. Method Budget's file with the app window redrawn as a stripboard, the budget flow, and the family strip (commented out, below) |
| `img/` | The pictures (`scripts/capture-screens.mjs`), light and dark |
| `assets/brand/` | Mark and wordmark SVGs, light and dark (`scripts/make-brand.py`) |
| `favicon.svg`, `favicon.ico`, `apple-touch-icon.png` | Browser and home-screen icons (`scripts/make-brand.py`) |
| `og.png`, `og-card.html`, `assets/og-card.css` | The 1200x630 share card and its source; `scripts/make-og.sh` renders it with headless Chrome (a throwaway profile, no window). Re-run after editing the card and commit all three |
| `robots.txt`, `sitemap.xml` | Crawlers: everything allowed; the sitemap lists `/` and `/releases/`. Add a line to the sitemap when a page is added |

Colors are tokens with a `prefers-color-scheme: dark` override, so the site follows the
visitor's system setting; every text pairing meets WCAG AA in both schemes (the table is
in BRAND.md, computed by `make-brand.py --contrast`). Layouts are mobile first and have
no horizontal scroll at phone width; the pictures scale to the column and show the light or
dark capture with the visitor's scheme. Each page has landmarks
(header, nav, main, footer), a skip link and visible focus outlines. Bands alternate
(`band`, `band alt`), and every card sets its background per band so it never vanishes
into the band behind it.

Links inside the pages are relative (except 404.html), so the site works at any address,
methodschedule.com or a local server. The canonical and share-card URLs name
methodschedule.com.

To look at it locally:

```bash
cd site && python3 -m http.server 8000    # then open http://127.0.0.1:8000/
```

### The family: the suite website's bar and band

methodschedule.com is served by the suite website, which puts a suite bar above every page
and a suite band at the bottom linking all four Method apps and methodproduction.com. Our
own pages therefore carry no family line or strip of their own: the footer's "Part of the
Method Production Suite" and "Also: Method Budget" lines were dropped on 2026-10-16 so the
band isn't repeated directly above it.

### Pictures

The home page and the README show the real app, not a drawing (SUITE-STANDARDS.md
section 7, decision 215). `node scripts/capture-screens.mjs` retakes every picture from
Field Station, the suite's invented production (`fixtures/field-station.mtsc`, made by
`method-schedule sample --field-station` from `fixtures/scripts/field-station.fountain`):

1. **board**: the stripboard, the first days of the 18-day shoot (the site's first picture, the README's hero);
2. **dood**: the day out of days;
3. **import**: the script just imported, its scenes in the Boneyard;
4. **cli**: the command line on the schedule file.

Each is 1280 by 800 logical pixels at 2x, light and dark, the window's content only. They
are drawn by headless Chrome from the app's own page (`harness/screens.html`), answering
the page's calls about the schedule with the engine's real answers
(`examples/screen_answers.rs`) and refusing any other, so a picture can only show what the
engine said; the command line picture shows the real command's output. Chrome draws in
software with whole-pixel text, so the same machine gives the same pixels every run.
Pictures go to `assets/screens/` for the README and `site/img/` for the site (the website
copies `site/` only), and a file is written only when its bytes change. Retaking them is a
release step ([RELEASING.md](RELEASING.md)).

### Try it in your browser

`/try/` is the app with the Field Station sample, running in the browser with nothing to
install (docs/research/web-session.md, phase 3). `cargo xtask web --demo --out site/try`
builds it: the page, the WebAssembly module, the shim, a note above the app saying it runs
in the browser (`crates/method-schedule-web/demo/`), and a Content-Security-Policy that
allows only the page's own files and compiling the module. Nothing is committed under
`site/try/`: the suite website's sync runs the build on the release it publishes (its
catalog's `build` line, as for Method Budget's demo), so the demo is always the released
engine. From 0.5.0 the site links it (the hero's "Try it in your browser") and the feature
list has it as built; MOrg adds the catalog's `build` line for that release's sync. Its
hardening: `?open=` loads only a schedule on the page's own site, and files are capped
before they are read.

## Publishing: the suite website

methodschedule.com is served by the Method Production Suite's one website, not from this
repository (since 2026-10-16, decision 209). The website repository,
`method-production-suite/website` (local checkout `../website`), copies this repository's
`site/` folder (`python3 tools/sync.py schedule` there), adds the suite bar above every
page and the suite band below, and deploys to Cloudflare Pages.

**The site follows the latest published release** (the owner's rule, 2026-10-18): from
v0.1.0 on, the sync copies `site/` from the newest published release tag, not from `main`,
so the site only describes what can be downloaded. A feature merged after a release
reaches the site with the next release. Before v0.1.0 the sync copied `main`.

So: change `site/` here (through `scripts/site.py` where it is generated) in the pull
request that makes the change; it goes live when the next release is published and MOrg
syncs. A fix the site needs sooner (a broken link) goes to MOrg, who owns the website
repository and decides how it goes out. This repository has no Pages workflow and no CNAME; the
domain's DNS lives with the website.

## For the owner: still to do

- **Make the repository public.** Until then the site's source, issue and discussion links
  only work for people with access.
- Discussions is on (2026-10-16); the site's Discussions links and the issue chooser's
  "Questions and ideas" link go there. The issue forms are in `.github/ISSUE_TEMPLATE/`
  (bug, feature, and a config with that link).
- **Help goes through GitHub, not email** (the owner, 2026-10-16, for the whole suite):
  bugs to Issues, questions to Discussions, private or business matters to
  hello@methodproduction.com. Method Schedule has no email address of its own.
- **A security policy** (`SECURITY.md`, or private vulnerability reporting turned on),
  then a "Security problems" contact link in `.github/ISSUE_TEMPLATE/config.yml`.
