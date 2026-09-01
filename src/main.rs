use std::collections::HashMap;
use std::ffi::OsString;
use std::io::{self, Stdout, Write};
use std::process::Command;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use clap::Parser;
use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::queue;
use crossterm::style::{Attribute, Color, Print, SetAttribute, SetForegroundColor};
use crossterm::terminal::{
    Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
    enable_raw_mode, size,
};
use rmux_fastcopy::{
    AppEvent, AppState, Hint, MatcherSet, Selection, build_popup_args, generate_hints,
    parse_pane_geometry, parse_show_options, run_action,
};
use unicode_width::UnicodeWidthChar;

const DEFAULT_ALPHABET: &str = "abcdefghijklmnopqrstuvwxyz";

#[derive(Debug, Parser)]
#[command(
    version,
    about = "Easymotion-style text copying for rmux",
    after_help = "Tab enters multi-select mode; Enter confirms; Esc cancels.\n\
                  Uppercase labels use --shift-action when configured."
)]
struct Args {
    /// Target rmux pane. The key binding passes #{pane_id} here.
    #[arg(long, env = "RMUX_FASTCOPY_PANE")]
    pane: String,

    /// Attached rmux client that should display the popup.
    #[arg(long, env = "RMUX_FASTCOPY_CLIENT")]
    client: Option<String>,

    /// rmux executable to use for pane capture and the default action.
    #[arg(long, default_value = "rmux")]
    rmux: String,

    /// Selection command. {} is replaced with the text; otherwise text is sent
    /// on stdin. Falls back to the @fastcopy-action option.
    #[arg(long)]
    action: Option<String>,

    /// Alternative selection command for uppercase labels. Falls back to the
    /// @fastcopy-shift-action option.
    #[arg(long)]
    shift_action: Option<String>,

    /// Unique characters used to form labels. Falls back to the
    /// @fastcopy-alphabet option.
    #[arg(long)]
    alphabet: Option<String>,

    /// Add, replace, or disable a matcher as NAME:PATTERN. Overrides the
    /// @fastcopy-regex-* option with the same name.
    #[arg(long = "regex", value_parser = parse_regex)]
    regexes: Vec<(String, String)>,

    /// Run the selector inside the popup created by the parent process.
    #[arg(long, hide = true)]
    popup_child: bool,
}

fn parse_regex(value: &str) -> Result<(String, String), String> {
    let Some((name, pattern)) = value.split_once(':') else {
        return Err("expected NAME:PATTERN".into());
    };
    if name.is_empty() {
        return Err("regex name must not be empty".into());
    }
    Ok((name.to_owned(), pattern.to_owned()))
}

fn main() {
    let original_args = std::env::args_os().collect::<Vec<_>>();
    if let Err(error) = run(Args::parse_from(&original_args), &original_args) {
        eprintln!("rmux-fastcopy: {error:#}");
        std::process::exit(1);
    }
}

fn run(args: Args, original_args: &[OsString]) -> Result<()> {
    if !args.popup_child {
        return open_popup(&args, original_args);
    }

    let text = capture_pane(&args.rmux, &args.pane)?;
    let options = load_options(&args.rmux);

    let alphabet = args
        .alphabet
        .clone()
        .or_else(|| option_value(&options, "@fastcopy-alphabet"))
        .unwrap_or_else(|| DEFAULT_ALPHABET.to_owned());

    let mut regex_overrides: Vec<(String, String)> = Vec::new();
    regex_overrides.extend(
        options
            .iter()
            .filter(|(name, _)| name.starts_with("@fastcopy-regex-"))
            .map(|(name, pattern)| {
                (
                    name.trim_start_matches("@fastcopy-regex-").to_owned(),
                    pattern.clone(),
                )
            }),
    );
    regex_overrides.extend(args.regexes.iter().cloned());

    let matchers = MatcherSet::with_overrides(regex_overrides)?;
    let hints = generate_hints(&alphabet, &text, &matchers.find(&text))?;
    let selection = run_ui(&text, &hints)?;
    let Some(selection) = selection else {
        return Ok(());
    };

    let action = if selection.shifted {
        args.shift_action
            .clone()
            .or_else(|| option_value(&options, "@fastcopy-shift-action"))
    } else {
        args.action
            .clone()
            .or_else(|| option_value(&options, "@fastcopy-action"))
    };
    let default_action = format!("{} load-buffer -", shell_words::quote(&args.rmux));
    if let Some(action) = action.or((!selection.shifted).then_some(default_action)) {
        run_action(&action, &selection, &args.pane)?;
    }
    Ok(())
}

fn open_popup(args: &Args, original_args: &[OsString]) -> Result<()> {
    let geometry = capture_format(
        &args.rmux,
        &args.pane,
        "#{pane_left} #{pane_top} #{pane_width} #{pane_height}",
    )?;
    let geometry = parse_pane_geometry(&geometry)?;
    let current_path = capture_format(&args.rmux, &args.pane, "#{pane_current_path}")?;
    let current_path = current_path.trim_end_matches(['\r', '\n']);
    let popup_args = build_popup_args(&args.pane, args.client.as_deref(), current_path, geometry);
    let executable = std::env::current_exe().context("locate rmux-fastcopy executable")?;
    let status = Command::new(&args.rmux)
        .args(popup_args)
        .arg(executable)
        .args(original_args.iter().skip(1))
        .arg("--popup-child")
        .status()
        .with_context(|| format!("run {:?} display-popup", args.rmux))?;
    if !status.success() {
        bail!("display-popup exited with {status}");
    }
    Ok(())
}

fn capture_format(rmux: &str, pane: &str, format: &str) -> Result<String> {
    let output = Command::new(rmux)
        .args(["display-message", "-p", "-t", pane, format])
        .output()
        .with_context(|| format!("run {rmux:?} display-message"))?;
    if !output.status.success() {
        bail!(
            "inspect pane {pane:?}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    String::from_utf8(output.stdout).context("pane format output is not UTF-8")
}

/// Load `@fastcopy-*` options from the rmux server, mirroring how
/// tmux-fastcopy reads `@fastcopy-regex-*` options from tmux. Failures are
/// non-fatal: the command line flags and built-in defaults still apply.
fn load_options(rmux: &str) -> Vec<(String, String)> {
    let output = match Command::new(rmux).args(["show-options", "-g"]).output() {
        Ok(output) => output,
        Err(error) => {
            eprintln!("rmux-fastcopy: load options: {error}");
            return Vec::new();
        }
    };
    if !output.status.success() {
        eprintln!(
            "rmux-fastcopy: load options: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
        return Vec::new();
    }
    let Ok(output) = String::from_utf8(output.stdout) else {
        eprintln!("rmux-fastcopy: load options: output is not UTF-8");
        return Vec::new();
    };
    parse_show_options(&output)
}

fn option_value(options: &[(String, String)], name: &str) -> Option<String> {
    options
        .iter()
        .find(|(existing, _)| existing == name)
        .map(|(_, value)| value.clone())
        .filter(|value| !value.is_empty())
}

fn capture_pane(rmux: &str, pane: &str) -> Result<String> {
    let output = Command::new(rmux)
        .args(["capture-pane", "-p", "-t", pane])
        .output()
        .with_context(|| format!("run {rmux:?} capture-pane"))?;
    if !output.status.success() {
        bail!(
            "capture pane {pane:?}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    String::from_utf8(output.stdout).context("captured pane is not UTF-8")
}

struct Terminal {
    stdout: Stdout,
}

impl Terminal {
    fn enter() -> Result<Self> {
        enable_raw_mode().context("enable raw terminal mode")?;
        let mut stdout = io::stdout();
        if let Err(error) = execute!(stdout, EnterAlternateScreen, Hide) {
            let _ = disable_raw_mode();
            return Err(error).context("enter alternate screen");
        }
        Ok(Self { stdout })
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        let _ = execute!(self.stdout, Show, LeaveAlternateScreen);
        let _ = disable_raw_mode();
    }
}

fn run_ui(text: &str, hints: &[Hint]) -> Result<Option<Selection>> {
    let mut terminal = Terminal::enter()?;
    let mut state = AppState::new(hints.to_vec());
    render(&mut terminal.stdout, text, hints, &state)?;

    loop {
        if !event::poll(Duration::from_secs(30)).context("poll terminal input")? {
            continue;
        }
        let event = event::read().context("read terminal input")?;
        match event {
            Event::Resize(_, _) => render(&mut terminal.stdout, text, hints, &state)?,
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                let app_event = match key.code {
                    KeyCode::Esc => return Ok(None),
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        return Ok(None);
                    }
                    KeyCode::Backspace => Some(AppEvent::Backspace),
                    KeyCode::Tab | KeyCode::BackTab => Some(AppEvent::Tab),
                    KeyCode::Enter => Some(AppEvent::Enter),
                    KeyCode::Char(mut ch) => {
                        if key.modifiers.contains(KeyModifiers::SHIFT) {
                            ch = ch.to_uppercase().next().unwrap_or(ch);
                        }
                        Some(AppEvent::Char(ch))
                    }
                    _ => None,
                };
                if let Some(app_event) = app_event {
                    if let Some(selection) = state.handle(app_event) {
                        return Ok(Some(selection));
                    }
                    render(&mut terminal.stdout, text, hints, &state)?;
                }
            }
            _ => {}
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CellStyle {
    Normal,
    Match,
    Skipped,
    Selected,
    Label,
    LabelTyped,
    DeselectLabel,
}

#[derive(Clone, Copy)]
struct Cell {
    byte_offset: usize,
    ch: char,
    style: CellStyle,
}

fn render(stdout: &mut Stdout, text: &str, hints: &[Hint], state: &AppState) -> Result<()> {
    let (width, height) = size().context("read terminal size")?;
    render_at_size(stdout, text, hints, state, width, height)
}

fn render_at_size<W: Write>(
    stdout: &mut W,
    text: &str,
    hints: &[Hint],
    state: &AppState,
    width: u16,
    height: u16,
) -> Result<()> {
    let mut cells = text
        .char_indices()
        .map(|(byte_offset, ch)| Cell {
            byte_offset,
            ch,
            style: CellStyle::Normal,
        })
        .collect::<Vec<_>>();
    let indexes = cells
        .iter()
        .enumerate()
        .map(|(index, cell)| (cell.byte_offset, index))
        .collect::<HashMap<_, _>>();

    for (hint_index, hint) in hints.iter().enumerate() {
        let selected = state.is_selected(hint_index);
        let candidate = hint.label.starts_with(state.input());
        for occurrence in &hint.occurrences {
            let match_style = if selected {
                CellStyle::Selected
            } else if candidate {
                CellStyle::Match
            } else {
                CellStyle::Skipped
            };
            for cell in cells.iter_mut().filter(|cell| {
                occurrence.selection.start <= cell.byte_offset
                    && cell.byte_offset < occurrence.selection.end
            }) {
                cell.style = match_style;
            }

            if !candidate && !selected {
                continue;
            }
            let Some(&start) = indexes.get(&occurrence.selection.start) else {
                continue;
            };
            for (label_offset, label_ch) in hint.label.chars().enumerate() {
                let Some(cell) = cells
                    .iter_mut()
                    .skip(start)
                    .filter(|cell| cell.ch != '\n' && cell.ch != '\r')
                    .nth(label_offset)
                else {
                    break;
                };
                cell.ch = label_ch;
                cell.style = if selected {
                    CellStyle::DeselectLabel
                } else if label_offset < state.input().chars().count() {
                    CellStyle::LabelTyped
                } else {
                    CellStyle::Label
                };
            }
        }
    }

    queue!(stdout, Clear(ClearType::All), MoveTo(0, 0))?;
    // Print the surface as a stream of text with SGR changes only at style
    // boundaries. rmux re-renders the popup on every refresh, so a compact
    // initial paint keeps the overlay frames (and the client's control
    // backlog) small; per-cell MoveTo/color sequences would flood the client
    // and trip rmux's attach-control prune.
    let mut x = 0_u16;
    let mut y = 0_u16;
    let mut active: Option<CellStyle> = None;
    for cell in cells {
        if y >= height {
            break;
        }
        if cell.ch == '\n' {
            if y.saturating_add(1) >= height {
                break;
            }
            queue!(stdout, Print("\r\n"))?;
            x = 0;
            y = y.saturating_add(1);
            active = None;
            continue;
        }
        if cell.ch == '\r' {
            queue!(stdout, Print("\r"))?;
            x = 0;
            continue;
        }
        let cell_width = cell.ch.width().unwrap_or(0) as u16;
        if cell_width > 0 && x.saturating_add(cell_width) > width {
            queue!(stdout, Print("\r\n"))?;
            x = 0;
            y = y.saturating_add(1);
            active = None;
        }
        if y >= height {
            break;
        }

        if active != Some(cell.style) {
            let (color, bold) = style(cell.style);
            queue!(
                stdout,
                SetForegroundColor(color),
                SetAttribute(if bold {
                    Attribute::Bold
                } else {
                    Attribute::NormalIntensity
                })
            )?;
            active = Some(cell.style);
        }
        queue!(stdout, Print(cell.ch))?;
        x = x.saturating_add(cell_width);
    }

    if hints.is_empty() && height > 0 {
        let message = "No copyable text found — Esc to close";
        queue!(
            stdout,
            MoveTo(0, height - 1),
            SetForegroundColor(Color::Yellow),
            SetAttribute(Attribute::Bold),
            Print(&message[..message.len().min(width as usize)])
        )?;
    }
    queue!(stdout, SetAttribute(Attribute::Reset))?;
    stdout.flush().context("draw fastcopy overlay")
}

fn style(style: CellStyle) -> (Color, bool) {
    match style {
        CellStyle::Normal => (Color::White, false),
        CellStyle::Match => (Color::Green, false),
        CellStyle::Skipped => (Color::DarkGrey, false),
        CellStyle::Selected => (Color::Yellow, false),
        CellStyle::Label => (Color::Red, true),
        CellStyle::LabelTyped => (Color::Yellow, true),
        CellStyle::DeselectLabel => (Color::DarkRed, true),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_does_not_advance_past_the_last_visible_row() {
        let hints = Vec::new();
        let state = AppState::new(hints.clone());
        let mut output = Vec::new();

        render_at_size(&mut output, "first\nsecond\n", &hints, &state, 80, 2).unwrap();

        assert_eq!(
            output.windows(2).filter(|bytes| *bytes == b"\r\n").count(),
            1,
            "a newline after the bottom row scrolls the popup surface"
        );
    }
}
