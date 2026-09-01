# rmux-fastcopy

`rmux-fastcopy` is a Rust implementation of the easymotion-style copying
workflow from [tmux-fastcopy](https://github.com/abhinav/tmux-fastcopy), built
for [rmux](https://rmux.io/docs/get-started/).

It captures the visible target pane and overlays short labels on useful text:

- IPv4 addresses
- Git commit hashes
- hexadecimal addresses and colors
- integers with at least four digits
- file paths
- UUIDs
- ISO dates

## Build and install

```sh
make check
make install
```

`make install` builds an optimized release binary and installs it as
`~/.cargo/bin/rmux-fastcopy`. Run `make help` to list the other build targets.

## Configure rmux

Open the overlay with `prefix + f`:

```tmux
bind f run-shell -C "run-shell -bE '$HOME/.cargo/bin/rmux-fastcopy --pane #{pane_id}'"
```

The default action writes the selection to an rmux buffer. On macOS, pass
`--action pbcopy` in the binding, or set `@fastcopy-action` to `pbcopy`, to
copy directly to the system clipboard instead.

`rmux-fastcopy` reads the target pane geometry and opens a popup over only that
pane, so the surrounding panes stay visible and the window does not appear to
zoom before selection. The pane ID keeps the overlay attached to the pane that
invoked the binding, including in split windows.
It uses rmux's pane-relative popup position so lower panes retain their vertical
offset instead of being clamped to the top of the window.
The outer `run-shell -C` is required for rmux 0.10 to expand that ID; the
inner background command starts fastcopy without blocking key processing.
`rmux-fastcopy` reads the target with `capture-pane` and never swaps, resizes,
splits, or otherwise changes windows, panes, or their layout.

## Use

1. Press `prefix + f`.
2. Type the red label over the text you want to copy.
3. Press `Esc` or `Ctrl-c` to cancel.

Press `Tab` before typing labels to enter multi-select mode. Type each label to
toggle it, then press `Enter` or `Tab` to copy all selected values separated by
spaces. Type an uppercase label to use `--shift-action`, if configured.

## Customize

The action is executed directly, not through a login shell. If it contains an
argument equal to `{}`, that argument is replaced with the selected text.
Otherwise the text is written to the command's standard input.

```sh
rmux-fastcopy --pane %1 --action 'rmux set-buffer {}'
rmux-fastcopy --pane %1 --action pbcopy
```

Add, replace, or disable named regular expressions with repeatable `--regex`
arguments. The first capture group, when present, is the copied portion.

```sh
rmux-fastcopy --pane %1 --regex 'ticket:ticket-(\d+)'
rmux-fastcopy --pane %1 --regex 'isodate:'
```

Actions receive `FASTCOPY_REGEX_NAME` and `FASTCOPY_TARGET_PANE_ID`, matching
the corresponding tmux-fastcopy behavior.

Run `rmux-fastcopy --help` for the complete CLI reference.

See [CONTRIBUTING.md](CONTRIBUTING.md) if you'd like to help improve the project.

## Configure via rmux options

Like tmux-fastcopy, `rmux-fastcopy` reads `@fastcopy-*` options from the rmux
server, so they can live in `~/.config/rmux/rmux.conf` next to the keybinding.
Command line flags always take precedence over these options.

```tmux
set-option -g @fastcopy-action 'rmux load-buffer -'
set-option -g @fastcopy-shift-action 'pbcopy'
set-option -g @fastcopy-alphabet asdfghjkl
set-option -g @fastcopy-regex-word "\\b[^\\s]+\\b"
```

Each `@fastcopy-regex-*` option adds a matcher named after the suffix. To copy
only a substring, put it in the first capture group; to disable a matcher,
set its option to an empty string.

```tmux
set-option -g @fastcopy-regex-phab-diff "\\bD\\d{3,}\\b"
set-option -g @fastcopy-regex-python-import "import ([\\w.]+)"
set-option -g @fastcopy-regex-isodate ""
```

Backslashes must be doubled inside the double quotes, exactly as in tmux and
rmux config files. Reload the config (`bind r` sources `~/.config/rmux/rmux.conf`
in the default setup) or restart the server after editing.
