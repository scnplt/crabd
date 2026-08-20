# crabd — terminal-based Docker resource manager (Rust, edition 2024)

TUI app built with ratatui + crossterm, async runtime tokio, Docker API via bollard.

## Commands

- Build: `cargo build`
- Test: `cargo test`
- Lint: `cargo clippy -- -D warnings`
- Format: `cargo fmt`
- Run: `cargo run` (interactive TUI; requires a reachable Docker daemon)

## Conventions

- **Language rule**: everything in this repository is written in English — code, identifiers, comments, documentation, commit messages, issues, PRs, and any other text. (Conversation with the maintainer may be in Turkish, but repository content is always English.)
- Keep clippy clean (`-D warnings`) and run `cargo fmt` before committing.
- Source layout: `src/docker/` (Docker/bollard integration), `src/ui/` (ratatui widgets), `src/main.rs` (entry point and event loop).

## Branching

`main` = production/release, `dev` = integration, every feature on its own
`<type>/<slug>` branch cut from `dev`. Never commit directly to `main` or `dev`.
Full model: `.claude/rules/git-workflow.md`. Contributor-facing version: `CONTRIBUTING.md`.

## Workflow

- For non-trivial features or refactors, plan first with the `planner` agent (Opus), then implement with the `implementer` agent (Sonnet), which must follow the approved plan.
