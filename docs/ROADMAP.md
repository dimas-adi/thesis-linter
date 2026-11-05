# Project Roadmap & Phase Exit Gates

Phase numbers restart at this migration. Commits before the `before-loop` tag use an older, unrelated numbering (its plan is in `docs/archive/`).

## Phase 0: Human Setup (PENDING)
**Goal:** Tag the pre-migration state, work on a branch, ignore real documents and runtime profiles, and commit the loop docs.
**Model:** Human.
**Exit gate:** `git tag --list before-loop` prints `before-loop`; `git branch --show-current` prints `loop-migration`; `git check-ignore -v thesis.docx profiles/x.toml` lists both paths; `git status --short` is empty after the commit.
**Human checks:** None.
**Commit:** `chore: migrate to loop workflow`

## Phase 1: Harness (ACTIVE)
**Goal:** Make `scripts/validate.sh` meet SPEC: Validation Contract, with a test-count baseline and a release smoke check. No production code changes.
**Model:** Sonnet, default effort.
**Exit gate:** `bash scripts/validate.sh` exits 0 and prints `baseline: 37 passed (baseline 37)` and `VALIDATE OK`; `git diff --stat -- src tests Cargo.toml Cargo.lock` is empty.
**Human checks:** None.
**Commit:** `chore(harness): enforce test baseline and release smoke in validate.sh`

## Phase 2: Core Engine Review (PENDING)
**Goal:** Review the CLI, profiles, linter, fixer, and report, then write fix tasks and missing-test tasks into Phase 3.
**Model:** Opus, effort high.
**Exit gate:** `bash scripts/validate.sh` exits 0; `docs/reviews/core.md` exists; Phase 3 in `docs/TASKS.md` has tasks; no file outside `docs/reviews/` and `docs/TASKS.md` changed.
**Human checks:** None. The owner reads `docs/reviews/core.md` before Phase 3.
**Commit:** `docs(review): core engine review`

## Phase 3: Core Engine Fixes (PENDING)
**Goal:** Apply the tasks from Phase 2 and add the missing core tests. This phase handles untrusted input and the owner's documents, so it runs at high effort.
**Model:** Sonnet, effort high.
**Exit gate:** `bash scripts/validate.sh` exits 0 with a raised baseline; no core rule in SPEC still ends with *(not yet tested)*.
**Human checks:** H-WORD, H-REAL.
**Commit:** `fix(core): apply core engine review findings`

## Phase 4: GUI Review (PENDING)
**Goal:** Review `DocxLintApp` and its panels, then write fix tasks and missing-test tasks into Phase 5.
**Model:** Opus, effort high.
**Exit gate:** `bash scripts/validate.sh` exits 0; `docs/reviews/gui.md` exists; Phase 5 in `docs/TASKS.md` has tasks; no file outside `docs/reviews/` and `docs/TASKS.md` changed.
**Human checks:** None. The owner reads `docs/reviews/gui.md` before Phase 5.
**Commit:** `docs(review): gui review`

## Phase 5: GUI Fixes (PENDING)
**Goal:** Apply the tasks from Phase 4 and add the missing GUI tests.
**Model:** Sonnet, default effort.
**Exit gate:** `bash scripts/validate.sh` exits 0 with a raised baseline; no rule in SPEC still ends with *(not yet tested)*.
**Human checks:** H-GUI, H-EXE.
**Commit:** `fix(gui): apply gui review findings`

New features: plan them with `/loop-feature`.
