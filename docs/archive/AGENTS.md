# Operating Principles
- **Working code only:** Never report a task or phase as done until `bash scripts/validate.sh` exits with code 0.
- **Zero manual testing requests:** NEVER ask the user to run `cargo`, click the GUI, or verify output manually. Keep all linting and profile logic decoupled from the GUI in `src/lib.rs` so 100% of features are tested automatically via `cargo test` in `scripts/validate.sh`.
- **Test harness first:** Before implementing any feature in `TASKS.md`, write or update the integration test in `tests/` first.
- **No Git commits:** NEVER run `git commit` or `git push`. Leave changes in the working tree for the human merge gate.

# Token Efficiency & In-Code Learning Rules (STRICT)
- **SILENT EXECUTION:** Do NOT write explanations, tutorials, or conversational summaries in chat.
- **Phase completion format:** When a phase finishes, reply in **under 10 lines** showing only the `scripts/validate.sh` pass summary and the updated `TASKS.md` checklist.
- **`// WHY:` Learning Comments:** Place a concise 1-line `// WHY:` comment above non-obvious Rust borrows, `quick-xml` streaming events, OpenXML conversions, or `egui` state updates so a beginner reading `git diff` understands *why* it works.

# Autonomous Phase Execution Loop
For the active phase in `TASKS.md`:
1. Read `SPEC.md`, `ROADMAP.md`, and `TASKS.md`.
2. For each unchecked `[ ]` task:
   - Write or update the integration test in `tests/` proving the acceptance criteria.
   - Implement the minimal Rust code in `src/`.
   - Run `bash scripts/validate.sh` and fix any errors autonomously until it exits 0.
   - Mark the task `[x]` in `TASKS.md`.
3. Once all tasks in the current phase are `[x]`, inspect `git diff` against `SPEC.md` and remove dead code or unused imports.
4. Re-run `bash scripts/validate.sh` one final time and stop for the Human Merge Gate.