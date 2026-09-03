//! The TUI picker: the carousel, drawn in the terminal.
//!
//! Same catalog as the GUI, same handoff: picking an entry runs the apply
//! command the environment names, or falls back to this binary's own apply.
//! Previews ride the terminal's image protocol where one exists - kitty,
//! sixel, iTerm2 - and degrade to unicode half-blocks anywhere else, so a
//! plain TTY still gets a picture rather than an error.

use std::collections::HashMap;
use std::io::IsTerminal;

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::layout::{Alignment, Constraint, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Paragraph};
use ratatui_image::picker::Picker;
use ratatui_image::protocol::StatefulProtocol;
use ratatui_image::{Resize, StatefulImage};

use crate::data::{self, Purpose, What};

/// What the user picked, if anything.
enum Exit {
    Chosen(String),
    Quit,
}

pub fn run(what: What, purpose: Purpose, state: Option<std::path::PathBuf>) -> Result<(), String> {
    if !std::io::stdout().is_terminal() {
        return Err("--tui needs a terminal; use --gui or pass a value".to_owned());
    }

    let state = match state {
        Some(dir) => dir,
        None => crate::paths::default_state_dir()?,
    };
    let rows = match what {
        What::Themes => data::theme_rows(),
        What::Backgrounds => data::background_rows(&state),
        What::Catalog => data::catalog_rows(
            &riso_core::reload::ProcessExecutor,
            crate::paths::DEFAULT_CATALOG,
        ),
    };
    if rows.is_empty() {
        return Err(match what {
            What::Themes => "no themes installed".to_owned(),
            What::Backgrounds => "the current theme ships no backgrounds".to_owned(),
            What::Catalog => "the catalog is empty or unreachable".to_owned(),
        });
    }
    let current = match what {
        What::Themes => data::current_theme(&state),
        What::Backgrounds => {
            data::current_background(&state).map(|p| p.to_string_lossy().into_owned())
        }
        What::Catalog => None,
    };

    // A terminal that answers the capability query gets real images; one
    // that does not gets half-blocks, which need only a font size.
    let picker = Picker::from_query_stdio().unwrap_or_else(|_| Picker::halfblocks());

    let mut terminal = ratatui::init();
    drain_pending_input();
    let outcome = browse(
        &mut terminal,
        &picker,
        &rows,
        current.as_deref(),
        what,
        purpose,
    );
    ratatui::restore();

    match outcome? {
        Exit::Quit => Ok(()),
        Exit::Chosen(value) => match purpose {
            Purpose::Browse => Ok(()),
            Purpose::Apply => apply(what, &value, &state),
            Purpose::Install => install(&value),
        },
    }
}

/// Install a catalog pick through this same binary, gate and all.
fn install(name: &str) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let status = std::process::Command::new(exe)
        .args(["theme", "install", name])
        .status()
        .map_err(|e| e.to_string())?;
    status
        .success()
        .then_some(())
        .ok_or_else(|| "installing the pick failed".to_owned())
}

/// Swallow whatever is already buffered on the tty.
///
/// The capability query above and the terminal's answers to it can leave
/// stray bytes behind, and crossterm would happily read them back as key
/// presses - the first of them an Escape, which would quit the picker
/// before it ever drew.
fn drain_pending_input() {
    while event::poll(std::time::Duration::from_millis(80)).unwrap_or(false) {
        let _ = event::read();
    }
}

/// The event loop: browse, filter, choose.
/// The picker's state between two frames.
struct Browse<'a> {
    rows: &'a [data::Row],
    what: What,
    purpose: Purpose,
    filter: String,
    selected: usize,
}

impl Browse<'_> {
    /// The rows the filter lets through, in order.
    fn shown(&self) -> Vec<usize> {
        let needle = self.filter.to_lowercase();
        self.rows
            .iter()
            .enumerate()
            .filter(|(_, row)| needle.is_empty() || row.label.to_lowercase().contains(&needle))
            .map(|(index, _)| index)
            .collect()
    }

    fn title(&self) -> &'static str {
        match (self.what, self.purpose) {
            (What::Catalog, _) => " riso - catalog ",
            (What::Themes, Purpose::Browse) => " riso - themes (browsing) ",
            (What::Themes, _) => " riso - themes ",
            (What::Backgrounds, Purpose::Browse) => " riso - backgrounds (browsing) ",
            (What::Backgrounds, _) => " riso - backgrounds ",
        }
    }

    fn hints(&self) -> String {
        if !self.filter.is_empty() {
            return format!("filter: {}  (Backspace edits, Esc clears)", self.filter);
        }
        let action = match self.purpose {
            Purpose::Apply => "Enter applies",
            Purpose::Browse => "Enter does nothing",
            Purpose::Install => "Enter installs",
        };
        format!("type to filter - arrows move - {action} - Esc quits")
    }

    fn name_line(&self, shown: &[usize]) -> Line<'static> {
        if shown.is_empty() {
            return Line::from("nothing matches");
        }
        let position = shown.iter().position(|i| *i == self.selected).unwrap_or(0);
        Line::from(format!(
            "< {} >  {}/{}",
            self.rows[self.selected].label,
            position + 1,
            shown.len()
        ))
        .style(Style::default().add_modifier(Modifier::BOLD))
    }

    fn draw(
        &self,
        frame: &mut ratatui::Frame,
        shown: &[usize],
        preview: Option<&mut StatefulProtocol>,
    ) {
        let [image_area, name_area, hint_area] = Layout::vertical([
            Constraint::Fill(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .margin(1)
        .areas(frame.area());
        frame.render_widget(Block::bordered().title(self.title()), frame.area());
        match preview {
            Some(protocol) => {
                let widget = StatefulImage::<StatefulProtocol>::new().resize(Resize::Fit(None));
                frame.render_stateful_widget(widget, image_area, protocol);
            }
            None => frame.render_widget(
                Paragraph::new("no preview").alignment(Alignment::Center),
                image_area,
            ),
        }
        frame.render_widget(
            Paragraph::new(self.name_line(shown)).alignment(Alignment::Center),
            name_area,
        );
        frame.render_widget(
            Paragraph::new(self.hints()).alignment(Alignment::Center),
            hint_area,
        );
    }

    /// The neighbour of the selection among the shown rows, wrapping.
    fn step(&self, shown: &[usize], forward: bool) -> usize {
        let Some(at) = shown.iter().position(|i| *i == self.selected) else {
            return self.selected;
        };
        let next = if forward {
            (at + 1) % shown.len()
        } else {
            (at + shown.len() - 1) % shown.len()
        };
        shown[next]
    }

    /// One key press; Some when the picker is done.
    fn on_key(&mut self, key: event::KeyEvent, shown: &[usize]) -> Option<Exit> {
        match key.code {
            KeyCode::Esc if !self.filter.is_empty() => self.filter.clear(),
            KeyCode::Esc => return Some(Exit::Quit),
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                return Some(Exit::Quit)
            }
            KeyCode::Enter if self.purpose != Purpose::Browse && !shown.is_empty() => {
                return Some(Exit::Chosen(self.rows[self.selected].value.clone()))
            }
            KeyCode::Left | KeyCode::Up if !shown.is_empty() => {
                self.selected = self.step(shown, false)
            }
            KeyCode::Right | KeyCode::Down | KeyCode::Tab if !shown.is_empty() => {
                self.selected = self.step(shown, true)
            }
            KeyCode::Backspace => {
                self.filter.pop();
            }
            KeyCode::Char(c) => self.filter.push(c),
            _ => {}
        }
        None
    }
}

fn browse(
    terminal: &mut ratatui::DefaultTerminal,
    picker: &Picker,
    rows: &[data::Row],
    current: Option<&str>,
    what: What,
    purpose: Purpose,
) -> Result<Exit, String> {
    // The capability handshake above can leave the terminal's answers
    // dribbling in past the drain, and crossterm degrades an unfinished
    // answer to a bare Escape: a quit nobody pressed. No human picks or
    // quits within half a second of launch, so early events are noise.
    let started = std::time::Instant::now();
    let settle = std::time::Duration::from_millis(500);

    let mut state = Browse {
        rows,
        what,
        purpose,
        filter: String::new(),
        selected: current
            .and_then(|value| rows.iter().position(|row| row.value == value))
            .unwrap_or(0),
    };
    let mut previews: HashMap<usize, Option<StatefulProtocol>> = HashMap::new();

    loop {
        let shown = state.shown();
        if !shown.contains(&state.selected) {
            state.selected = shown.first().copied().unwrap_or(0);
        }
        let preview = shown
            .contains(&state.selected)
            .then(|| {
                previews.entry(state.selected).or_insert_with(|| {
                    load_preview(picker, rows[state.selected].preview.as_deref())
                })
            })
            .and_then(|slot| slot.as_mut());

        terminal
            .draw(|frame| state.draw(frame, &shown, preview))
            .map_err(|e| e.to_string())?;

        let ev = event::read().map_err(|e| e.to_string())?;
        if started.elapsed() < settle {
            continue;
        }
        if let Event::Key(key) = ev {
            if key.kind != KeyEventKind::Release {
                if let Some(exit) = state.on_key(key, &shown) {
                    return Ok(exit);
                }
            }
        }
    }
}

fn load_preview(picker: &Picker, path: Option<&std::path::Path>) -> Option<StatefulProtocol> {
    let image = image::ImageReader::open(path?).ok()?.decode().ok()?;
    Some(picker.new_resize_protocol(image))
}

/// Hand the pick over, exactly the way the GUI would.
///
/// The environment can name the apply command, which is how a machine's own
/// wrapper carries the theme to whatever shells it runs; without one the
/// binary applies through itself, so the picker works out of the box.
fn apply(what: What, value: &str, state: &std::path::Path) -> Result<(), String> {
    let (env, fallback_arg) = match what {
        What::Themes => ("RISO_CAROUSEL_APPLY", "theme"),
        What::Backgrounds => ("RISO_CAROUSEL_APPLY_BG", "backgrounds"),
        // Catalog picks install, and never through apply.
        What::Catalog => return install(value),
    };

    if let Ok(command) = std::env::var(env) {
        if !command.trim().is_empty() {
            let status = std::process::Command::new("sh")
                .arg("-c")
                .arg(format!("{command} \"$1\""))
                .arg("riso-tui")
                .arg(value)
                .status()
                .map_err(|e| e.to_string())?;
            return status
                .success()
                .then_some(())
                .ok_or_else(|| format!("{command} failed"));
        }
    }

    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let status = std::process::Command::new(exe)
        .args([fallback_arg, "set", value, "--state"])
        .arg(state)
        .status()
        .map_err(|e| e.to_string())?;
    status
        .success()
        .then_some(())
        .ok_or_else(|| "applying the pick failed".to_owned())
}
