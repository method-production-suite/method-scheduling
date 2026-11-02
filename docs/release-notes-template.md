# Writing release notes

Every version has notes in `docs/releases/<version>.md`, written once and shown in three
places, so they are written for the people who schedule films (the suite's template,
SUITE-STANDARDS.md section 7.2, from Method Budget's):

- **In the app.** After an update Method Schedule shows the notes once, and Help > What's
  New shows them again. The app draws headings, paragraphs, bullets, **bold** and `code`;
  nothing else.
- **On GitHub.** The release workflow uses the file as the release's description.
- **On the website.** The newest notes fill the "What's new" box and every version is on
  the release notes page (`scripts/site.py`).

Start a new file from the skeleton at the end of this page.
`python3 scripts/notes-check.py <version>` says whether it is ready to publish. Release
steps: [RELEASING.md](RELEASING.md).

## Voice

- Write for someone scheduling a film, not for someone reading the code. Say what they can
  do now, or what works better, in their words: "Print a call sheet for any shoot day", not
  "callsheet flow in report.js".
- Plain sentences. No em dashes (a colon, a comma or a period instead). No exclamation
  marks. No internal names: no crate, module, function or file names, no commit hashes, no
  roadmap or decision numbers.
- Examples use invented productions only (Field Station is the suite's). Never a real
  production, a real person or a real schedule: the notes become a public web page.
- Claim only what is built. If `docs/features.toml` or `docs/ROADMAP.md` doesn't back a
  claim, leave it out.
- No other product's name, and no comparisons.
- Windows signing, in Known gaps, always in these words: "Not signed, and no certificate
  is planned: Windows warns that the publisher is unknown before it opens." The installer
  stays unsigned for good, so never "not signed yet".
- Most commits earn no bullet. Refactors, tests, tooling and documentation don't, unless a
  scheduler would notice.

## Shape

The same in every Method app, in this order:

1. `# Method Schedule <version>`, the title. It ends with the version.
2. One headline paragraph: one or two sentences on what this release means.
3. **What's new**: one or more sections, each `## ` and a heading named for what changed for
   the reader ("Call sheets you can print", not "Call sheet printing"). Any visible change
   gets a picture: a capture from `node scripts/capture-screens.mjs`, linked as
   `../../assets/screens/<name>-light.png`, or a short clip.
4. `## Fixed`: what works now that didn't. Leave it out when nothing was fixed.
5. `## Known gaps`: what doesn't work yet that someone might try, and which systems are
   untested. Always there.
6. `## Thanks`: everyone who helped with this release by name (or handle), film knowledge
   included: the scheduler who explained a rule counts as much as the person who wrote the
   code. Always there.

In each section, bullets. A bullet may start with a bold claim, `**Like this.**`, followed
by a sentence or two.

## Length

A minor release: a headline, two to four sections of what's new, up to about a dozen
bullets. A patch release: a headline, `## Fixed`, and the two closing sections. The file
stays well under 60,000 characters (a GitHub release has a limit).

## The skeleton

Copy the block below into `docs/releases/<version>.md` and fill in the version. Every
`{{...}}` is a placeholder: `notes-check.py` refuses the file until none is left.

<!-- skeleton:start -->
```markdown
# Method Schedule {{version}}

{{One or two sentences: what this release means for someone scheduling a film.}}

## {{A heading named for what changed for the reader}}

- **{{The claim, in the reader's words.}}** {{What it means, and where to find it.}}

## Fixed

- **{{What works now.}}** {{What used to go wrong, in a few words.}}

## Known gaps

- {{What doesn't work yet, or which systems nobody has installed on yet.}}

## Thanks

- {{Everyone who helped, by name or handle, and what they helped with, film knowledge included.}}
```
<!-- skeleton:end -->
