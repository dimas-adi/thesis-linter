# docx-lint

A standalone Windows tool (`docx-lint.exe`) that lints academic `.docx` theses against a
configurable university profile, and can auto-fix the mechanical problems.

- **GUI** (run with no arguments / double-click): pick or drop a `.docx`, edit profile
  settings, browse violations by category, export a fixed `.docx` or an HTML report.
- **CLI** (run with arguments): headless, used by `scripts/validate.sh` and automated tests.

## Build

```bash
cargo build --release      # target/release/docx-lint.exe
bash scripts/validate.sh   # fmt + clippy + tests + release build; must print VALIDATE OK
```

## CLI usage

```bash
docx-lint check thesis.docx                       # exit 0 only when there are no violations
docx-lint check thesis.docx --json                # machine-readable output
docx-lint check thesis.docx --fix fixed.docx      # write an auto-fixed copy, re-lint it
docx-lint check thesis.docx --html report.html    # HTML report
docx-lint check thesis.docx --profile other_kampus

docx-lint config init other_kampus                # create a profile with defaults
docx-lint config set other_kampus page.top_cm 3.0
docx-lint config use other_kampus
docx-lint config show
```

Use `--profiles-dir <dir>` to point at a different profiles folder.

## Profiles

Profiles live in `./profiles/<name>.toml`; `./profiles/active_profile.txt` names the active one.
They cover page margins, font/size/line spacing, chapter style (`BAB I` vs `BAB 1`), heading
alignment, caption prefixes and separator, the bibliography heading, forbidden tokens, and
foreign terms that must be italic. See `docs/SPEC.md` for the full schema.

Extra foreign terms can be listed one per line in `./profiles/foreign_terms.txt`
(`#` starts a comment); they are merged with the profile's list in both CLI and GUI.

## What it checks

| Code | Meaning |
| --- | --- |
| `MARGIN_ERROR` | Page margin differs from the profile (within tolerance). |
| `FONT_ERROR` | Wrong font name or size. Front matter and tables have relaxed sizes. |
| `SPACING_ERROR` | Line spacing differs from the profile. |
| `HEADING_ERROR` | Chapter numbering style or heading alignment is wrong. |
| `CAPTION_SEQ_ERROR` | Caption separator, chapter number or running number is wrong; numbering restarts at 1 each chapter. |
| `CITATION_ORPHAN` | An `(Author, 2020)` citation has no entry under the bibliography heading. |
| `DRAFT_TOKEN_ERROR` | A forbidden whole word such as `TODO` appears. |
| `DOUBLE_SPACE_ERROR` | Consecutive spaces in text. |
| `FOREIGN_TERM_NOT_ITALIC` | A listed foreign term is not italic. |

Chapters are recognised either from heading text (`BAB I ...`) or from auto-numbered
`Heading1` paragraphs, where Word generates the "BAB n" label. Everything before the first
chapter (cover, lists of contents/figures/tables) is treated as front matter: it gets relaxed
font sizes and its caption entries are ignored.

## Auto-fix

`--fix` rewrites margins, fonts, sizes, line spacing and double spaces, and splits runs so
foreign terms become their own italic runs. It handles runs with several `<w:t>` elements,
tabs/breaks inside runs, and runs inside hyperlinks. It never overwrites the input file.

## Docs

`docs/AGENTS.md` (working rules), `docs/SPEC.md`, `docs/ROADMAP.md`, `docs/TASKS.md`.
