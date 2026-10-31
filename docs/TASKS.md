# Active Tasks Checklist

## Phase 1: Rust Scaffolding, Profile Config & Layout/Font Linter (DONE)
- [x] **T1.1:** Initialize `Cargo.toml` (`zip`, `quick-xml`, `serde`, `toml`, `serde_json`, `clap`, `anyhow`, `tempfile`) and create `scripts/validate.sh` (`cargo fmt --check && cargo clippy -- -D warnings && cargo test`).
- [x] **T1.2:** Implement `src/config.rs` with `ProfileConfig` structs and `docx-lint config` subcommands (`init`, `set`, `use`, `show`) storing `.toml` presets in `./profiles/`.
- [x] **T1.3:** Create synthetic `.docx` test builder helper in `tests/common/mod.rs` that packs custom `word/document.xml` strings into valid `.docx` ZIP archives.
- [x] **T1.4:** Implement streaming `quick-xml` parser in `src/linter.rs` for `MARGIN_ERROR` (twips to cm), `FONT_ERROR`, and `SPACING_ERROR`, driven dynamically by `ProfileConfig`.
- [x] **T1.5:** Wire up `docx-lint check <file> [--profile] [--json]` in `src/main.rs` and verify all Phase 1 tests pass via `bash scripts/validate.sh`.

## Phase 2: Dynamic Structure, Citations, HTML Report & `--fix` Engine (DONE)
- [x] **T2.1:** Write failing tests and implement `HEADING_ERROR` (`roman` vs `arabic` chapters) and per-chapter `CAPTION_SEQ_ERROR` (`.` vs `-` separators) in `src/linter.rs`.
- [x] **T2.2:** Write failing tests and implement `CITATION_ORPHAN`, `DRAFT_TOKEN_ERROR`, `DOUBLE_SPACE_ERROR`, and `FOREIGN_TERM_NOT_ITALIC`.
- [x] **T2.3:** Write failing tests and implement `src/fixer.rs` (`--fix <output.docx>`) and `src/report.rs` (`--html <report.html>`), verifying via `bash scripts/validate.sh` that fixed documents pass re-linting.

## Phase 3: Native Desktop GUI (`eframe`/`egui`) & `.exe` Packaging (DONE)
- [x] **T3.1:** Add `eframe` (with `glow` backend) and `rfd` to `Cargo.toml`, and create `src/gui.rs` with a testable `DocxLintApp` state struct (active profile, loaded `.docx` path, violation results, filter tab, status banner).
- [x] **T3.2:** Implement the GUI panels in `src/gui.rs`: Top toolbar (File picker + Drag-and-drop + Profile switcher), Left settings editor (margins, font, `BAB I` vs `BAB 1` dropdown, Save button), and Central violations table + "Auto-Fix & Save Copy" / "Export HTML" buttons.
- [x] **T3.3:** Update `src/main.rs` so running `docx-lint.exe` with 0 arguments launches the native `eframe` desktop GUI, while passing CLI args runs the headless CLI. Add unit tests for `DocxLintApp` state actions and update `scripts/validate.sh` to verify `cargo build --release` succeeds.

## Phase 4: Real-World Thesis Fixes & Run-Splitting Auto-Fixer (DONE)
- [x] **T4.1 (Regex Word Boundaries):** In `src/structure.rs`, enforce `\b` word boundaries for `forbidden_tokens` (matching whole words like `\bTODO\b` case-insensitively) so words like "METODOLOGI" are never flagged.
- [x] **T4.2 (Exempt Front Matter & Tables from 12pt/1.5):** 
  - Exempt paragraphs before the first Chapter heading (`BAB I` / `BAB 1`) from strict 12pt checks (allow 12pt, 14pt, or 16pt for Title/Cover).
  - Detect paragraphs inside tables (`<w:tbl>`) and exempt them from 12pt font and 1.5 line spacing (allow 10pt/11pt and 1.0 spacing).
- [x] **T4.3 (Front Matter Caption Exemption & Per-Chapter Reset):** Ignore caption sequence checks before the first Chapter heading (skipping `DAFTAR GAMBAR` / `DAFTAR TABEL`), and ensure caption sequence numbers strictly reset to 1 for each new chapter.
- [x] **T4.4 (External Vocabulary Dictionary):** Support an external dictionary file at `./profiles/foreign_terms.txt` (merging it with `foreign_terms_must_italicize`), and add an input box in the GUI settings panel to add/edit words.
- [x] **T4.5 (Run-Splitting Italic Fixer):** In `src/fixer.rs`, implement OpenXML run-splitting for `FOREIGN_TERM_NOT_ITALIC` so un-italicized foreign words are split into isolated `<w:r>` runs containing `<w:rPr><w:i/></w:rPr>`. Add tests in `tests/` proving re-linting a fixed document brings foreign term violations to 0.