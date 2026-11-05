# Active Tasks Checklist

## Phase 0: Human Setup (PENDING, Human)
Run by the human. Agents never run these steps.
- [ ] **T0.1** [human] (SPEC: Decisions, D-004): Run `git tag before-loop`, then `git switch -c loop-migration`. Proof: `git tag --list before-loop` prints `before-loop`; `git branch --show-current` prints `loop-migration`.
- [ ] **T0.2** [human] (SPEC: Protected Files, D-005): Add two lines to `.gitignore`: `*.docx` and `/profiles/`. Proof: `git check-ignore -v thesis.docx profiles/x.toml` lists both paths.
- [ ] **T0.3** [human] (SPEC: Decisions, D-004): Check that the old docs are staged as renames. Proof: `git status --short` shows `R` for `docs/AGENTS.md`, `docs/ROADMAP.md`, `docs/SPEC.md`, `docs/TASKS.md`, and `GEMINI.md`, all into `docs/archive/`.
- [ ] **T0.4** [human] (SPEC: Validation Contract): Run `git add -A`, then `git commit -m "chore: migrate to loop workflow"`. Proof: `git status --short` is empty.

## Phase 1: Harness (DONE, Sonnet)
Scripts and `scripts/baseline.env` only. Do not change `src/`, `tests/`, `Cargo.toml`, or `Cargo.lock`.
- [x] **T1.1** [sonnet] (SPEC: Validation Contract): Create `scripts/check-baseline.sh <passed-count>` and set `BASE_TEST_PASS=37` in `scripts/baseline.env`. The script reads the value with `grep` and strips `\r`, because `core.autocrlf=true` can turn this file to CRLF. It exits 2 if the value is missing or the argument is not an integer; exits 1 and prints `baseline: <count> passed, expected at least <baseline>` if the count is lower; otherwise exits 0 and prints `baseline: <count> passed (baseline <baseline>)`. Proof: `bash scripts/check-baseline.sh 36` exits 1, `bash scripts/check-baseline.sh 37` exits 0, and `bash scripts/check-baseline.sh abc` exits 2.
- [x] **T1.2** [sonnet] (SPEC: Validation Contract): In `scripts/validate.sh`, capture the `cargo test` output to a `mktemp` file with `tee`, keeping `set -euo pipefail` so a failing test still stops the script. Sum `N` from every `test result: ok. N passed` line and pass the sum to `scripts/check-baseline.sh`. Remove the temp file with a `trap`. Keep `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo build --release` unchanged and in order. Proof: `bash scripts/validate.sh` exits 0 and prints `baseline: 37 passed (baseline 37)` and `VALIDATE OK`; `grep -cE 'cargo fmt --check|cargo clippy --all-targets -- -D warnings|cargo build --release' scripts/validate.sh` prints `3`.
- [x] **T1.3** [sonnet] (SPEC: Validation Contract): In `scripts/validate.sh`, after `cargo build --release` and before `VALIDATE OK`, run `target/release/docx-lint --help` with stdout discarded, then print `smoke: release binary OK`. Proof: `bash scripts/validate.sh` exits 0 and prints `smoke: release binary OK`; `git diff --stat -- src tests Cargo.toml Cargo.lock` is empty.

## Phase 2: Core Engine Review (PENDING, Opus)
- [ ] **T2.1** [opus] (R-002, R-004, R-005, R-006, R-009, R-010, R-011, R-016, R-020, R-021, R-028, R-037, R-039, R-042, R-044, R-046, R-048): Review `src/main.rs`, `src/config.rs`, `src/linter.rs`, `src/structure.rs`, `src/fixer.rs`, `src/report.rs`, and their tests, per AGENTS: Review Loop. Measure at least: R-010 with aliased output paths (`./in.docx`, different letter case, a hard link); paragraph-index agreement between the streaming linter and `parse_paragraphs` for `<w:p>` nested in text boxes (R-006); and hostile archives against R-009. Write fix tasks into Phase 3, plus one test task with a named test for each rule cited here; each test task also authorizes changing that rule's marker in `docs/SPEC.md` to *(existing test: <name>)*. Proof: `docs/reviews/core.md` exists with the findings table, and Phase 3 in `docs/TASKS.md` has one task per High or Medium finding and one test task per cited rule.

## Phase 3: Core Engine Fixes (PENDING, Sonnet)
Tasks are written by T2.1.

## Phase 4: GUI Review (PENDING, Opus)
- [ ] **T4.1** [opus] (R-010, R-056): Review `src/gui.rs` and `tests/gui_state.rs`, per AGENTS: Review Loop. Measure at least Auto-Fix with an output path that aliases the loaded document (R-010). Write fix tasks into Phase 5, plus one test task with a named test for each rule cited here that is still *(not yet tested)*; each test task also authorizes changing that rule's marker in `docs/SPEC.md` to *(existing test: <name>)*. Proof: `docs/reviews/gui.md` exists with the findings table, and Phase 5 in `docs/TASKS.md` has one task per High or Medium finding and one test task per rule still untested.

## Phase 5: GUI Fixes (PENDING, Sonnet)
Tasks are written by T4.1.
