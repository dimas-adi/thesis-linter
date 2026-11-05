# Project Specification: `docx-lint` (Standalone `.exe` GUI + CLI Academic `.docx` Linter)

## 1. Overview
`docx-lint` compiles into a single portable Windows executable (`docx-lint.exe`).
- **GUI Mode (Default when launched with 0 arguments / double-clicked):** Opens a native desktop window (`eframe` / `egui`) where users can pick or drag-and-drop a `.docx` file, visually edit and save University Profile settings (`BAB I` vs `BAB 1`, margins, fonts), browse violations by category, and click a button to export a fixed `.docx` or HTML report.
- **CLI Mode (When arguments are passed):** Runs headlessly in the terminal so automated test scripts (`scripts/validate.sh`) can verify every feature without human interaction.

## 2. Tech Stack
- **Language:** Rust (compiles to standalone `.exe`)
- **Core Linter Crates:** `zip`, `quick-xml`, `serde`, `toml`, `serde_json`, `clap`, `anyhow`
- **Desktop GUI Crates:** `eframe` (with `glow` renderer for lightweight `.exe` compatibility), `egui`, `rfd` (native Windows Open/Save file dialogs)

## 3. Persistent University Profile Config (`ProfileConfig`)
Stored as `.toml` files in `./profiles/<profile_name>.toml` (with `./profiles/active_profile.txt` tracking the active profile).

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
chapter_prefix = "BAB"                # e.g., "BAB", "CHAPTER"
chapter_numbering = "roman"           # "roman" (BAB I, II) | "arabic" (BAB 1, 2)
heading_alignment = "center"          # "center" | "left"
figure_prefix = "Gambar"
table_prefix = "Tabel"
caption_separator = "."               # "." (Gambar 1.1) | "-" (Gambar 1-1)
bibliography_heading = "DAFTAR PUSTAKA"

[academic]
forbidden_tokens = ["TODO", "TBD", "cek lagi", "lorem ipsum"]
foreign_terms_must_italicize = ["framework", "backend", "frontend", "smartphone", "database", "cache"]