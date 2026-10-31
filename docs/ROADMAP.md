
# Project Roadmap & Phase Exit Gates

## Phase 1: Rust Core, Profile Config Manager & Layout/Typography Linter
**Goal:** Set up Cargo project, `scripts/validate.sh`, synthetic `.docx` ZIP/XML test builder, `ProfileConfig` persistence, and streaming XML checks for `MARGIN_ERROR`, `FONT_ERROR`, and `SPACING_ERROR`.
**Automated Exit Gate:**
- `bash scripts/validate.sh` runs `cargo fmt --check`, `cargo clippy -- -D warnings`, and `cargo test` with exit code `0`.
- Integration tests verify `docx-lint config` persists settings to `./profiles/*.toml` and `docx-lint check --json` accurately flags margin, font, and line-spacing errors.

## Phase 2: Dynamic Structure (`BAB I` vs `BAB 1`), Captions, Citations & Auto-Fixer
**Goal:** Implement `HEADING_ERROR`, `CAPTION_SEQ_ERROR`, `CITATION_ORPHAN`, `DRAFT_TOKEN_ERROR`, `DOUBLE_SPACE_ERROR`, `FOREIGN_TERM_NOT_ITALIC`, plus `--fix` and `--html` exporters.
**Automated Exit Gate:**
- `bash scripts/validate.sh` passes all tests verifying that switching a profile from `roman`/`.` to `arabic`/`-` dynamically changes validation behavior on a `BAB 1` / `Gambar 1-1` document, and `--fix` clears `FONT_ERROR` and `DOUBLE_SPACE_ERROR`.


## Phase 3: Native Windows Desktop GUI (`eframe` / `egui`) & Standalone `.exe` Build
**Goal:** Build the native desktop GUI (`src/gui.rs`) launched when `docx-lint.exe` runs with no CLI arguments, featuring file picker/drag-and-drop, visual University Profile editor, violation inspector table, and one-click Fix/HTML export buttons.
**Automated Exit Gate:**
- `bash scripts/validate.sh` runs all unit/integration tests, headless GUI state transition tests (`GuiAppState` load file, change profile setting, save profile, trigger fix), and verifies `cargo build --release` produces `target/release/docx-lint.exe` (or platform binary) with 0 warnings.