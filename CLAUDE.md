# crabd — terminal-based Docker resource manager (Rust, edition 2024)

TUI app built with ratatui + crossterm, async runtime tokio, Docker API via bollard.

## Commands

- Build: `cargo build`
- Test: `cargo test`
- Lint: `cargo clippy -- -D warnings`
- Format: `cargo fmt`
- Run: `cargo run` (interactive TUI; requires a reachable Docker daemon)

## Conventions

- Work on the `dev` branch; PRs target `main`.
- Keep clippy clean (`-D warnings`) and run `cargo fmt` before committing.
- Source layout: `src/docker/` (Docker/bollard integration), `src/ui/` (ratatui widgets), `src/main.rs` (entry point and event loop).

## Workflow

- For non-trivial features or refactors, plan first with the `planner` agent (Opus), then implement with the `implementer` agent (Sonnet), which must follow the approved plan.
