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
        None => crate::default_state_dir()?,
    };
    let rows = match what {
        What::Themes => data::theme_rows(),
        What::Backgrounds => data::background_rows(&state),
        What::Catalog => {
            data::catalog_rows(&riso_core::reload::ProcessExecutor, crate::DEFAULT_CATALOG)
        }
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

    let mut filter = String::new();
    let mut selected = current
        .and_then(|value| rows.iter().position(|row| row.value == value))
        .unwrap_or(0);
    let mut previews: HashMap<usize, Option<StatefulProtocol>> = HashMap::new();

    loop {
        let shown: Vec<usize> = rows
            .iter()
            .enumerate()
            .filter(|(_, row)| {
                filter.is_empty() || row.label.to_lowercase().contains(&filter.to_lowercase())
            })
            .map(|(index, _)| index)
            .collect();
        if !shown.contains(&selected) {
            selected = shown.first().copied().unwrap_or(0);
        }

        let preview = shown.contains(&selected).then(|| {
            previews
                .entry(selected)
                .or_insert_with(|| load_preview(picker, rows[selected].preview.as_deref()))
        });

        terminal
            .draw(|frame| {
                let title = match (what, purpose) {
                    (What::Catalog, _) => " riso - catalog ",
                    (What::Themes, Purpose::Browse) => " riso - themes (browsing) ",
                    (What::Themes, _) => " riso - themes ",
                    (What::Backgrounds, Purpose::Browse) => " riso - backgrounds (browsing) ",
                    (What::Backgrounds, _) => " riso - backgrounds ",
                };
                let [image_area, name_area, hint_area] = Layout::vertical([
                    Constraint::Fill(1),
                    Constraint::Length(1),
                    Constraint::Length(1),
                ])
                .margin(1)
                .areas(frame.area());

                frame.render_widget(Block::bordered().title(title), frame.area());

                match preview {
                    Some(Some(protocol)) => {
                        let widget =
                            StatefulImage::<StatefulProtocol>::new().resize(Resize::Fit(None));
                        frame.render_stateful_widget(widget, image_area, protocol);
                    }
                    _ => frame.render_widget(
                        Paragraph::new("no preview").alignment(Alignment::Center),
                        image_area,
                    ),
                }

                let name = if shown.is_empty() {
                    Line::from("nothing matches")
                } else {
                    let position = shown.iter().position(|i| *i == selected).unwrap_or(0);
                    Line::from(format!(
                        "< {} >  {}/{}",
                        rows[selected].label,
                        position + 1,
                        shown.len()
                    ))
                    .style(Style::default().add_modifier(Modifier::BOLD))
                };
                frame.render_widget(Paragraph::new(name).alignment(Alignment::Center), name_area);

                let hints = if filter.is_empty() {
                    let action = match purpose {
                        Purpose::Apply => "Enter applies",
                        Purpose::Browse => "Enter does nothing",
                        Purpose::Install => "Enter installs",
                    };
                    format!("type to filter - arrows move - {action} - Esc quits")
                } else {
                    format!("filter: {filter}  (Backspace edits, Esc clears)")
                };
                frame.render_widget(
                    Paragraph::new(hints).alignment(Alignment::Center),
                    hint_area,
                );
            })
            .map_err(|e| e.to_string())?;

        let ev = event::read().map_err(|e| e.to_string())?;
        if started.elapsed() < settle {
            continue;
        }
        match ev {
            Event::Key(key) if key.kind != KeyEventKind::Release => {
                let step = |from: usize, forward: bool| -> usize {
                    let Some(at) = shown.iter().position(|i| *i == from) else {
                        return from;
                    };
                    let next = if forward {
                        (at + 1) % shown.len()
                    } else {
                        (at + shown.len() - 1) % shown.len()
                    };
                    shown[next]
                };
                match key.code {
                    KeyCode::Esc if !filter.is_empty() => filter.clear(),
                    KeyCode::Esc => return Ok(Exit::Quit),
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        return Ok(Exit::Quit)
                    }
                    KeyCode::Enter if purpose != Purpose::Browse && !shown.is_empty() => {
                        return Ok(Exit::Chosen(rows[selected].value.clone()))
                    }
                    KeyCode::Left | KeyCode::Up if !shown.is_empty() => {
                        selected = step(selected, false)
                    }
                    KeyCode::Right | KeyCode::Down | KeyCode::Tab if !shown.is_empty() => {
                        selected = step(selected, true)
                    }
                    KeyCode::Backspace => {
                        filter.pop();
                    }
                    KeyCode::Char(c) => filter.push(c),
                    _ => {}
                }
            }
            _ => {}
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
