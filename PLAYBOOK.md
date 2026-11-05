# Loop Workflow Playbook

For the human and for any AI that helps plan: the agent during `/loop-init`, `/loop-migrate`, or `/loop-feature`, or a planning chat on claude.ai. Coding agents in a phase only need `docs/AGENTS.md`.

## Roles
- **Human (owner):** decides scope, answers product questions, runs the Human-Only Checks, reads `git diff`, commits. Never writes code by hand during a phase.
- **Planner (chat AI or a `/loop-*` planning command):** turns intent into SPEC, ROADMAP, and TASKS; reviews phase reports; writes follow-up tasks. Does not code.
- **Agent (a Claude Code session or an Antigravity conversation):** runs one phase per session, proves everything through `scripts/validate.sh`, never commits.

## The cycle
1. Plan: SPEC (rules with IDs, decisions), ROADMAP (phases with gates), TASKS (tasks with refs and proofs). `/loop-analyze` checks the three files against each other, then the human commits them.
2. Run: new session with the phase's model, `/loop-phase N`.
3. Gate: the agent reports in under 10 lines. The human runs `/loop-status` and the listed human checks, then commits with the ROADMAP message.
4. Repeat. After a block of features, a fresh `[opus]` session reviews (one numbered phase) and a `[sonnet]` session fixes (the next numbered phase).

One phase = one session = one commit. Small phases keep diffs readable and failures cheap. Harness permissions and git hooks stop agents from committing, pushing, or reading secrets.

## Writing tasks
This is the part that saves the most tokens. A good task has:
- **Refs:** the rule IDs it implements (`R-007`), or a SPEC section (`SPEC: Validation Contract`), or a review finding (`S2, High`). Every rule needs at least one task or an existing test; `/loop-analyze` checks both directions.
- **Behavior and boundary:** what changes, where, and what must not change.
- **Proof:** new test names, or a command with expected output (`grep ... is empty`, counts equal). The agent names new tests exactly as the Proof says, so `/loop-analyze` can find them later.
- **Model tag:** `[sonnet]` by default; `[haiku]` only for mechanical edits with no security effect; `[opus]` for reviews. A tag also sets the effort (AGENTS: Model Routing); raise a phase to `effort high` in ROADMAP for security, concurrency, or data migrations.
- **Order:** security fixes, then deletions, then restructuring, so each refactor runs on a smaller, tested base.

Avoid:
- Tasks that contradict another task or SPEC (agents follow SPEC and mark `[!] conflict`).
- "Improve X" without a measurable end.
- Mixing backend and frontend in one session when both change: run backend first, make it green, then frontend.
- Haiku tasks that touch auth, tokens, parsing of untrusted input, or concurrency.

Write each task for the model that will run it:
- **`[sonnet]`** follows instructions literally. If a rule applies to every item (every endpoint, every screen, every table), say "every" or list them; otherwise the agent applies it only to the item named. Name exact files, functions, or routes.
- **`[haiku]`** (and Flash): one file or one small area, a flat instruction, and one example of the expected change when the format matters. No judgment calls.
- **`[opus]`**: describe the outcome and the boundary, not the steps. Do not add "double-check everything" or extra verifier steps; the Proof and `validate.sh` are the check.
- For every tag: one outcome per task, a file or area anchor, positive wording ("return 429 with `sisaDetik`") over long lists of prohibitions, and exact numbers instead of adjectives.

When the planner is unsure how the code behaves, the task should say "first record in the task output how X works today; if it is not Y, mark `[!] needs owner decision`" instead of guessing.

## Clarifying requirements
Ambiguity is cheapest to fix before a phase runs. When planning, scan the request against these categories and mark each Clear, Partial, or Missing:

1. **Users and roles:** who uses it; who may see or change what.
2. **Scope:** what "done" means for this release; what is out of scope (goes to SPEC Non-Goals).
3. **Data:** entities, required fields, uniqueness, status changes, deletion, retention.
4. **Flows and states:** the main journeys; error, empty, loading, and offline states.
5. **Security and privacy:** login, sessions, whose data each role sees, secrets, protected files.
6. **Limits and numbers:** sizes, counts, timeouts, rate limits. Vague words (fast, secure, many, user-friendly) must become numbers or observable results.
7. **Integrations:** external services, file formats, and what happens when they fail.
8. **Edge cases and concurrency:** double submits, two users editing, retries, time zones.
9. **Constraints:** platform, host OS, stack, offline use, deadlines.
10. **Terms:** one name per concept, matching the UI language.
11. **Human-only checks:** what only a person can verify (goes to SPEC Human-Only Checks).

Then:
- Ask only about Partial and Missing categories, highest impact first: security and data before cosmetics. Recommend an answer for each question.
- Decide yourself what has a safe, conventional default, and say so in the reply.
- Record every answer as one line in SPEC Decisions (`D-###`, date, question, answer, affected rules).
- If the human chooses to decide later, write `[NEEDS CLARIFICATION: <question>]` where the answer belongs. Agents mark dependent tasks `[!] needs owner decision` instead of guessing.

## Migrating an existing project
1. Tag the current state (`git tag before-loop`) and work on a branch.
2. Archive old planning docs into `docs/archive/` (gitignored); code, schema, and tests become the only sources of truth.
3. Phase 1 builds the harness without touching production code. If existing analysis shows warnings, list them, do not fix them yet.
4. Review + fix pairs per area (backend, then frontend) before new features, so features land on a clean base.
5. Then feature phases, then a security review of the new diff, then release.

## Reviews
- Always a new `[opus]` session, never the one that wrote the code.
- Measured proof beats reasoning: throwaway scripts outside the repo, parallel requests for races, timing for enumeration.
- Output: `docs/reviews/<scope>.md` with a "Checked, no finding" list and one findings table; fix tasks go straight into TASKS as their own numbered phase.
- The human reads the review before running fixes. The planner may adjust tasks (e.g. extend a fix to a sibling case the review missed).

## When things go wrong
- **Red before start:** fix the environment (missing local env, service down), not the code.
- **`[!]` after 3 attempts:** new `[opus]` session, `/loop-task Tn.m`; then `/loop-phase N` in a new session finishes the rest.
- **Session interrupted** (crash, context limit, closed terminal): new session, `/loop-phase N`. It resumes from the first open task.
- **`[!] needs owner decision`:** `/loop-analyze <phase>` asks the question, records the answer in SPEC Decisions, and updates the task. Then `/loop-phase N`.
- **Human check fails:** reproduce it in a test first. The bug becomes a task with a regression test (`/loop-feature "bug: ..."`).
- **Out of time:** use the ROADMAP cut line: keep what is green, record the rest as a limitation.

## Planning chat on claude.ai
Copy this when you start:
```
I use a loop workflow for coding with an agent (Claude Code or Antigravity): docs/AGENTS.md (rules), docs/SPEC.md (contract),
docs/ROADMAP.md (phases with exit gates), docs/TASKS.md (tasks with model tags and proofs), one phase
per session, the agent stops only when bash scripts/validate.sh exits 0, and I commit at each gate.
PLAYBOOK.md is attached. Help me plan: <goal>. Ask at most 4 questions, then give me the doc changes
as copy-ready text. Talk to me in Indonesian; docs in English.
```
