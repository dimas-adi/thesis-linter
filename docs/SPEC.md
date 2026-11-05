# Project Specification: `docx-lint`

Standalone Windows tool that lints academic `.docx` theses against a configurable university profile and auto-fixes the mechanical problems. Code, schema, and tests are the sources of truth; this file describes what they do as of the migration on 2026-11-05.

## Language & Conventions
- **Human language:** Indonesian in chat and reports. **Project language:** English for code, comments, docs, GUI text, and violation messages.
- **Terms:** *profile* (one university's rules), *chapter* (a `BAB` heading), *front matter* (everything before the first chapter), *caption* (`Gambar`/`Tabel` line), *violation* (one finding with a code), *dictionary* (`profiles/foreign_terms.txt`).
- **Format:** `cargo fmt` defaults; clippy warnings are errors. Comments follow AGENTS: `WHY:` Comments.
- **Tests:** integration tests in `tests/`, one file per area. Documents are synthetic, built by `tests/common/mod.rs`, inside `tempfile` directories.
- **GUI logic:** every GUI action is a public method on `DocxLintApp` with no `egui` type in its signature, so `cargo test` covers it without a window.

## Stack
- **Language:** Rust edition 2024 (toolchain seen at migration: rustc and cargo 1.98.1; no `rust-toolchain` file).
- **Core crates:** `zip` 9.2 (deflate only), `same-file` 1.0, `quick-xml` 0.42, `serde` 1.0, `toml` 1.1, `serde_json` 1.0, `clap` 4.6 (derive), `anyhow` 1.0.
- **GUI crates:** `eframe` 0.37 (`glow`, `default_fonts`), `rfd` 0.17.
- **Dev crates:** `tempfile` 3.27.
- **Maintenance:** `.github/dependabot.yml` opens weekly Cargo update PRs. There is no CI pipeline.
- **Host:** Windows 11, Git Bash for scripts, `core.autocrlf=true`. `.gitattributes` keeps `*.sh` at LF.

## Layout
- `src/main.rs`: argument parsing, CLI/GUI switch, exit codes.
- `src/config.rs`: `ProfileConfig`, `ProfileStore`, dictionary.
- `src/linter.rs`: margin, font, and spacing checks; entry points `lint_file` and `lint_xml`.
- `src/structure.rs`: paragraph parsing; heading, caption, citation, draft-token, double-space, and italic checks; `Ctx` exemptions.
- `src/fixer.rs`: `fix_file`, `fix_xml`, italic run splitting.
- `src/report.rs`: HTML report.
- `src/gui.rs`: `DocxLintApp` state, actions, and `egui` panels.
- `tests/`: `check_cli`, `config_cli`, `structure_cli`, `academic_cli`, `fix_report_cli`, `gui_state`, `phase4_cli`, `common/mod.rs`.
- `scripts/validate.sh`: the gate. `docs/reviews/`: review output. `docs/archive/`: old planning docs.
- `./profiles/` is runtime user data relative to the working directory. It is not part of the repo.

## Profile Schema
Stored as `./profiles/<profile_name>.toml`. `./profiles/active_profile.txt` names the active profile. `./profiles/foreign_terms.txt` holds extra foreign terms, one per line, with `#` comments.

```toml
profile_name = "default_kampus"

[page]
top_cm = 4.0
left_cm = 4.0
bottom_cm = 3.0
right_cm = 3.0
tolerance_cm = 0.05

[typography]
font_name = "Times New Roman"
body_size_pt = 12.0
line_spacing = 1.5

[structure]
chapter_prefix = "BAB"
chapter_numbering = "roman"     # "roman" (BAB I) | "arabic" (BAB 1)
heading_alignment = "center"    # "center" | "left"
figure_prefix = "Gambar"
table_prefix = "Tabel"
caption_separator = "."         # "." (Gambar 1.1) | "-" (Gambar 1-1)
bibliography_heading = "DAFTAR PUSTAKA"

[academic]
forbidden_tokens = ["TODO", "TBD", "cek lagi", "lorem ipsum"]
foreign_terms_must_italicize = ["framework", "backend", "frontend", "smartphone", "database", "cache"]
```

## Protected Files
Never open, print, or copy these (AGENTS: Hard Limits):
- `.env`, `.env.*`, `scripts/validate.local.env`.
- `**/*.docx` in the repo tree: real thesis documents hold personal data. Synthetic documents that tests create in temp directories are not protected.

The project has no secrets today. `scripts/validate.sh` needs no local env file.

## Validation Contract
`bash scripts/validate.sh` is the only gate. It runs, in this order, and stops at the first failure:
1. `cargo fmt --check`
2. `cargo clippy --all-targets -- -D warnings`
3. `cargo test`, then `scripts/check-baseline.sh` compares the summed passed count with `BASE_TEST_PASS` in `scripts/baseline.env` *(check added by Phase 1)*
4. `cargo build --release`
5. `target/release/docx-lint --help` exits 0 *(smoke check added by Phase 1)*
6. Prints `VALIDATE OK`

State and isolation: tests write only inside `tempfile` directories, use no network, and never read real documents or `./profiles/` of the repo. Baseline at migration: 37 passing tests.

## Invariants

### CLI contract
- **R-001:** With at least one argument the binary runs the headless CLI and opens no window. Launch with 0 arguments is covered by H-EXE. *(existing test: clean_document_passes)*
- **R-002:** `check` exits 0 with no violations, 1 with at least one violation, and 2 on any error. `config` exits 0 on success and 2 on error. *(existing test: exit_codes_follow_contract)*
- **R-003:** With `--fix`, the exit code reflects the violations left in the fixed copy, not those in the input. *(existing test: fix_reports_remaining_unfixable)*
- **R-004:** `check --json` prints one JSON object with `file`, `profile`, and `violations`; each violation has `code`, `message`, and `paragraph`. With `--fix` it also has `fixed_file` and `remaining`. *(existing test: json_output_has_contract_fields)*
- **R-005:** The violation codes are exactly `MARGIN_ERROR`, `FONT_ERROR`, `SPACING_ERROR`, `HEADING_ERROR`, `CAPTION_SEQ_ERROR`, `CITATION_ORPHAN`, `DRAFT_TOKEN_ERROR`, `DOUBLE_SPACE_ERROR`, and `FOREIGN_TERM_NOT_ITALIC`. *(existing test: violation_codes_are_the_documented_set)*
- **R-006:** `paragraph` is the 1-based index of the `<w:p>` in document order, counting empty `<w:p/>`; it is 0 for document-level findings such as margins. *(existing test: paragraph_index_counts_every_w_p_in_document_order)*
- **R-007:** A document that satisfies the profile produces zero violations. *(existing test: clean_document_passes)*
- **R-008:** A missing input file ends the run with a non-zero exit code. *(existing test: missing_file_errors)*
- **R-009:** An input that is not a zip, has no `word/document.xml`, has malformed XML, or has a `word/document.xml` larger than 64 MiB decompressed ends with an `error:` message on stderr and exit code 2, never a panic. *(existing test: hostile_archives_exit_2_without_panic)*

### Auto-fix
- **R-010:** `--fix` and GUI Auto-Fix never modify the input `.docx`, including when the output path is another spelling of the same file; such a request is an error. `--html` never overwrites the input either. *(existing test: fix_never_modifies_input_bytes)*
- **R-011:** The fixed copy contains every other package entry of the input with identical decompressed content; only `word/document.xml` is rewritten. *(existing test: fix_preserves_other_package_entries_byte_for_byte)*
- **R-012:** After `--fix`, re-linting reports no `MARGIN_ERROR`, `FONT_ERROR`, `SPACING_ERROR`, or `DOUBLE_SPACE_ERROR`, and XML entities in text stay escaped. *(existing test: fix_clears_fixable_violations_and_keeps_package_parts)*
- **R-013:** `--fix` puts every non-italic foreign term in its own italic run, keeps the run's other formatting, and leaves the paragraph text unchanged. *(existing test: fixer_splits_runs_so_foreign_terms_become_italic)*
- **R-014:** Italic splitting handles runs with several `<w:t>`, tabs, breaks, and page-break markers, and runs inside hyperlinks. *(existing test: fixer_handles_multi_child_runs_and_hyperlinks)*
- **R-015:** Violations the fixer does not repair stay in `remaining`. *(existing test: fix_reports_remaining_unfixable)*
- **R-016:** The fixer applies the same exemptions as the linter (R-021, R-022, R-023), so it leaves cover sizes and table sizes and spacing unchanged. *(existing test: fixer_keeps_cover_and_table_exemptions)*
- **R-057:** `--fix` output is atomic: on failure an existing output stays byte-identical and no temp file remains. *(existing test: failed_fix_keeps_existing_output_and_leaves_no_temp_file)*
- **R-058:** Equations (`m:oMath`, `m:oMathPara`) are excluded from font and size checks and fixes. *(existing test: equation_fonts_are_neither_flagged_nor_rewritten)*

### Layout checks
- **R-017:** `MARGIN_ERROR` fires when a `pgMar` side (twips / 567) differs from the profile by more than `tolerance_cm`. *(existing test: margin_error_flagged_and_tolerance_respected)*
- **R-018:** `FONT_ERROR` fires for an `ascii` or `hAnsi` font other than `font_name`, and for a run size outside the allowed sizes. *(existing test: font_error_name_and_size)*
- **R-019:** `SPACING_ERROR` fires when `line` / 240 differs from `line_spacing` by more than 0.01. *(existing test: spacing_error_flagged)*
- **R-020:** Line spacing is checked only when `lineRule` is `auto` or absent. *(existing test: spacing_checked_only_for_auto_line_rule)*
- **R-021:** Paragraphs styled `Heading*` or `Title` skip the size check but keep the font-name check. *(existing test: heading_styles_skip_size_but_keep_font_check)*
- **R-022:** When a document has a chapter, paragraphs before the first chapter also allow 12, 14, and 16 pt; paragraphs after it allow only `body_size_pt`. *(existing test: front_matter_allows_cover_sizes_but_body_stays_strict)*
- **R-023:** Paragraphs inside `<w:tbl>` also allow 10 and 11 pt and 1.0 spacing; the exemption ends with the table. *(existing test: table_paragraphs_allow_small_font_and_single_spacing)*

### Structure checks
- **R-024:** `HEADING_ERROR` fires when a chapter number does not match `chapter_numbering`: canonical uppercase roman or ASCII digits. *(existing test: roman_headings_ok_arabic_flagged)*
- **R-025:** Switching a profile from `roman`/`.` to `arabic`/`-` changes the result for the same document. *(existing test: switching_profile_changes_validation)*
- **R-026:** `HEADING_ERROR` fires when a chapter heading's alignment differs from `heading_alignment`; a styled heading with no `jc` is skipped, because its alignment comes from `styles.xml`. *(existing test: heading_alignment_checked)*
- **R-027:** A sentence that starts with the chapter prefix but has no chapter number, or whose word after the number starts in lowercase, is not a heading. *(existing test: sentence_starting_with_prefix_is_not_a_heading)*
- **R-028:** A table-of-contents line (`...`, or a trailing page number after the title) is not a chapter heading. *(existing test: toc_lines_are_not_chapter_headings)*
- **R-029:** A heading split by a line break (`BAB I<br/>PENDAHULUAN`) still counts as a chapter. *(existing test: chapter_heading_split_by_line_break_still_resets_captions)*
- **R-030:** An auto-numbered `Heading1` counts as the next chapter; `numId` 0 turns that off. It counts only when no paragraph in the document has a typed chapter number. *(existing test: auto_numbered_heading1_counts_as_chapter_for_captions)*
- **R-031:** `CAPTION_SEQ_ERROR` fires for a wrong separator, a wrong chapter number, or a skipped running number, counted separately for figures and tables. A line whose word after the number starts in lowercase is a body sentence, not a caption. *(existing test: captions_numbered_per_chapter)*
- **R-032:** Captions before the first chapter are ignored, and numbering restarts at 1 in each chapter. *(existing test: front_matter_captions_skipped_and_numbering_resets_per_chapter)*

### Academic checks
- **R-033:** `CITATION_ORPHAN` fires for an `(Author, 2020)` citation, including `;`-separated lists, when no entry under `bibliography_heading` contains both author and year. *(existing test: citation_orphans_flagged)*
- **R-034:** With no bibliography heading every citation is orphaned; parentheses that are not citations are ignored. *(existing test: citation_without_bibliography_is_orphan_and_plain_parens_ignored)*
- **R-035:** `DRAFT_TOKEN_ERROR` matches `forbidden_tokens` as whole words, case-insensitively, so `METODOLOGI` never matches `TODO`. *(existing test: draft_tokens_match_whole_words_only)*
- **R-036:** `DOUBLE_SPACE_ERROR` fires for consecutive spaces within a run and across adjacent runs. *(existing test: double_space_within_and_across_runs)*
- **R-037:** Tabs and line breaks never create a false `DOUBLE_SPACE_ERROR`. *(existing test: tabs_and_breaks_never_create_double_space)*
- **R-038:** `FOREIGN_TERM_NOT_ITALIC` fires for a whole-word term in a non-italic run; italic runs and longer words (`frameworks`) pass. *(existing test: foreign_terms_need_italic)*
- **R-039:** `<w:i w:val="0"/>` and `w:val="false"` count as not italic in both the linter and the fixer. *(existing test: italic_off_values_count_as_not_italic)*

### Profiles
- **R-040:** `config init`, `set`, `use`, and `show` persist to `<profiles-dir>/<name>.toml` and `active_profile.txt`. *(existing test: init_set_use_show_persist_to_profiles_dir)*
- **R-041:** `config set` rejects an unknown key or a value of the wrong type, and `config use` rejects a missing profile. *(existing test: set_rejects_unknown_key_and_missing_profile_use)*
- **R-059:** `config set`, `config init`, and loading reject values outside the Profile Schema (non-finite numbers, negative margins or tolerance, zero sizes or spacing, unknown enum values) with exit 2. *(existing test: config_set_rejects_values_outside_schema)*
- **R-060:** `config init` never replaces an existing profile. *(existing test: config_init_refuses_existing_profile)*
- **R-042:** A profile name that is empty or contains `/`, `\`, `.`, `:`, `<`, `>`, `"`, `|`, `?`, `*`, or a control character is rejected, so a profile path never leaves the profiles directory. *(existing test: invalid_profile_names_are_rejected)*
- **R-043:** Profile resolution order is the named profile, then the active profile, then built-in defaults named `default_kampus`. *(existing test: profile_drives_validation)*
- **R-044:** A profile file written with the schema in Profile Schema keeps loading with the same values; keys are never renamed or removed. *(existing test: schema_profile_loads_with_same_values)*
- **R-045:** Dictionary words merge into the foreign terms for CLI and GUI, ignoring blank lines and `#` comments, without duplicates, and are never written back into the `.toml`. *(existing test: external_dictionary_is_merged_for_cli_and_gui)*
- **R-046:** `--profiles-dir <dir>` replaces `./profiles` for every subcommand. *(existing test: profiles_dir_applies_to_every_subcommand)*

### HTML report
- **R-047:** The report lists every violation in a table and HTML-escapes all document-derived text. *(existing test: html_report_lists_and_escapes_violations)*
- **R-048:** The report is one self-contained file with no external URLs, and a clean run shows `No violations found.` *(existing test: html_report_is_self_contained_and_reports_clean_run)*

### GUI state
- **R-049:** Loading a document fills `violations` and the status text. *(existing test: load_file_populates_violations_and_status)*
- **R-050:** Loading a bad path clears `violations` and sets a status starting with `Error`. *(existing test: load_missing_file_sets_error_banner)*
- **R-051:** Filter tabs partition the violations; a code outside Layout and Structure appears under Academic. *(existing test: filter_tabs_partition_violations)*
- **R-052:** Saving a profile persists it and re-lints the loaded document. *(existing test: save_profile_persists_and_relints)*
- **R-053:** Switching profiles loads the profile and makes it active. *(existing test: switch_profile_loads_and_activates)*
- **R-054:** Auto-Fix and Export HTML write their output files. *(existing test: autofix_and_export_html_write_files)*
- **R-055:** Auto-Fix and Export HTML with no document set a status starting with `Error`. *(existing test: actions_without_document_report_error)*
- **R-056:** Saving the dictionary replaces only words from the old dictionary; terms typed into the profile survive. *(not yet tested)*

## Non-Goals
- Checking anything other than `word/document.xml`: headers, footers, footnotes, and `styles.xml` are out of scope (D-002).
- Auto-fixing headings, captions, citations, or draft tokens: these are only reported (D-002).
- Official support for any OS other than Windows (D-002).
- Network access, telemetry, or a CI pipeline; the quality gate is the local `scripts/validate.sh` (D-002).

## Human-Only Checks
- **H-WORD:** A file produced by `--fix` opens in real Microsoft Word with no repair prompt, and its layout looks intact.
- **H-GUI:** The window, panels, drag-and-drop, and the Windows Open and Save dialogs work on a real screen.
- **H-EXE:** Double-clicking `docx-lint.exe` from another folder, on a machine without the Rust toolchain, opens the GUI.
- **H-REAL:** A real thesis, kept outside the repo, gives no false positives the owner considers wrong.

## Decisions
- **D-001 (2026-11-05):** What must not change? The CLI contract, input-file safety, the profile format, and the launch mode. Rules: R-001 to R-006, R-010, R-040, R-044, R-045.
- **D-002 (2026-11-05):** What is out of scope? Other document parts, structural auto-fix, other operating systems, and network or CI. See Non-Goals.
- **D-003 (2026-11-05):** What only a human can check? Real Word, the real GUI, the double-clicked `.exe`, and a real thesis. Checks: H-WORD, H-GUI, H-EXE, H-REAL.
- **D-004 (2026-11-05):** Which old docs are archived? The old `docs/SPEC.md`, `ROADMAP.md`, `TASKS.md`, `AGENTS.md`, and root `GEMINI.md` move to `docs/archive/`. `README.md` stays.
- **D-005 (2026-11-05, planner default):** Real thesis `.docx` files are protected and `*.docx` and `/profiles/` are gitignored, because tests need neither and `git add -A` would otherwise commit personal data. The owner may override this. Rules: Protected Files.
