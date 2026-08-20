# Contributing

Thanks for your interest in crabd! Contributions are welcome. The notes below
keep the history clean and reviews cheap.

## Before you write code

For anything more than a small fix, open an issue first so the change can be
discussed before you invest time in it.

## Language

**Everything in this repository is English.** Code comments, doc comments,
identifiers, log and error messages, markdown, commit messages, branch names,
PR titles and bodies, issue text. This holds regardless of the language a
discussion happens in.

## Branching

| Branch | Role |
|---|---|
| `main` | Production / release. Tagged `vX.Y.Z`. |
| `dev` | Integration. The default base for pull requests. |
| `<type>/<slug>` | One feature or fix. Short-lived, deleted after merge. |

Never commit directly to `main` or `dev`. Cut from `dev`, return to `dev`:

```bash
git checkout dev && git pull
git checkout -b feat/my-change
# ... commits ...
git push -u origin feat/my-change
gh pr create --base dev
```

Branch prefixes reuse the commit types: `feat/`, `fix/`, `refactor/`, `docs/`,
`test/`, `chore/`, `perf/`, `ci/`.

## Commits

```
<type>: <description>

<optional body explaining why, not what>
```

Types: `feat`, `fix`, `refactor`, `docs`, `test`, `chore`, `perf`, `ci`.

Keep commits small enough to review on their own. The body is for the reason a
change is correct — the diff already says what changed.

## Gates

Every one of these must pass before a pull request is ready:

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo build
cargo test
```

Testing UI changes also means running the TUI (`cargo run`) against a real
Docker daemon and exercising the affected screens.

## License

By contributing you agree your work is licensed under the
[Apache License 2.0](LICENSE.txt), the same as the rest of the project.
