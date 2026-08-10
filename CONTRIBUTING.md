# Contributing to rmux-fastcopy

Thanks for your interest in contributing! This project follows the conventions
of the surrounding [rmux](https://rmux.io/docs/get-started/) ecosystem and of
the original [tmux-fastcopy](https://github.com/abhinav/tmux-fastcopy), which
it reimplements.

## Ways to contribute

- **Report bugs** — open an issue with the reproduction steps, the `rmux-fastcopy`
  version (`rmux-fastcopy --version`), and your rmux/terminal setup.
- **Suggest features** — open an issue describing the use case before writing code.
- **Fix bugs and add features** — fork the repository, make your change, and open
  a pull request.

## Prerequisites

- Rust toolchain (edition 2024 is used, so a recent stable Rust is required)
- `cargo` and the usual Rust tooling (`rustfmt`, `clippy`)
- [rmux](https://rmux.io/docs/get-started/) for manual end-to-end testing

## Project layout

```
src/
  lib.rs          Core logic: capture, matching, overlay, action handling
  main.rs         CLI entry point and argument parsing
tests/
  core.rs         Unit and integration tests for matching and actions
  layout_safety.rs  Tests asserting the overlay never changes the layout
```

## Development workflow

Clone the repository and build:

```sh
git clone https://github.com/tenfyzhong/rmux-fastcopy
cd rmux-fastcopy
cargo build
```

Run the full check suite (formatting, lint, and tests):

```sh
make check
```

Or run the steps individually:

```sh
make fmt        # auto-format sources
make fmt-check  # verify formatting
make lint       # clippy with -D warnings
make test       # cargo test
make build      # debug build
make release    # optimized release build
```

Before opening a pull request, make sure `make check` passes locally.

## Code style

- Run `make fmt` before committing; the CI check is `make fmt-check`.
- Keep `clippy` clean: `make lint` fails on any warning.
- Prefer small, focused commits over one large change.
- Follow the naming and error-handling conventions already present in `src/`.

## Testing

- Add or update tests in `tests/` for new matching behavior and for changes to
  the action pipeline.
- `tests/layout_safety.rs` is especially important: `rmux-fastcopy` promises it
  never swaps, resizes, splits, or otherwise changes windows, panes, or their
  layout. Keep that promise covered.
- For interactive changes, test manually in a real rmux session using the
  keybinding from the README.

## Opening a pull request

1. Fork the repository and create a feature branch.
2. Make your change with focused commits.
3. Run `make check` and fix any failures.
4. Open the pull request and describe what changed and why.

## Commit messages

Write clear, imperative commit messages that explain the *why*:

```
Add support for the --regex flag

The flag lets users register custom named matchers, replacing the
hard-coded list. The first capture group, when present, is copied.
```

## License

By contributing, you agree that your contributions are licensed under the
[GNU General Public License v2](LICENSE).
