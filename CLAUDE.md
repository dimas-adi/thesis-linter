# Loop workflow

This repo is built with the loop workflow. Before any work, read `docs/AGENTS.md`; it points to `docs/SPEC.md`, `docs/ROADMAP.md`, and `docs/TASKS.md`. Those four files override anything else.

- Talk to the human in Indonesian. Code, comments, and docs in English.
- Never commit, push, tag, or switch branches. The human commits at the end of each phase.
- Never read `.env`, `.env.*`, `scripts/validate.local.env`, or the protected files listed in `docs/SPEC.md`. Your harness permission settings and the git hooks enforce these rules; if a command is denied or blocked, report it and never retry it in another form.
- The only definition of done is `bash scripts/validate.sh` exiting 0.
- The workflow's slash commands start with `/loop-` (`/loop-status` shows what to do next).
