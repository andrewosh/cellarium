use std::io;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use ratatui::{
    crossterm::{
        event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
        execute,
        terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
    },
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, Paragraph},
    prelude::CrosstermBackend, Terminal,
};

pub struct ParamState {
    pub values: Vec<f32>,
    pub defaults: Vec<f32>,
    pub names: Vec<String>,
    pub selected: usize,
    pub title: String,
    pub running: bool,
}

pub type SharedParams = Arc<Mutex<ParamState>>;

pub fn save_params(state: &ParamState) -> io::Result<String> {
    let mut entries: Vec<String> = Vec::new();
    for (name, &val) in state.names.iter().zip(state.values.iter()) {
        entries.push(format!("  \"{}\": {}", name, val));
    }
    let json = format!("{{\n{}\n}}", entries.join(",\n"));

    let filename = format!("{}_params.json", state.title.to_lowercase().replace(' ', "_"));
    std::fs::write(&filename, &json)?;
    Ok(filename)
}

pub fn load_params(state: &mut ParamState, path: &Path) -> io::Result<()> {
    let contents = std::fs::read_to_string(path)?;
    let parsed: serde_json::Value = serde_json::from_str(&contents)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    if let serde_json::Value::Object(map) = parsed {
        for (name, val) in &map {
            if let Some(idx) = state.names.iter().position(|n| n == name) {
                if let Some(v) = val.as_f64() {
                    state.values[idx] = v as f32;
                }
            }
        }
    }
    Ok(())
}

pub fn spawn(shared: SharedParams) -> JoinHandle<()> {
    thread::spawn(move || {
        if let Err(e) = run_tui(&shared) {
            eprintln!("TUI error: {}", e);
        }
    })
}

fn run_tui(shared: &SharedParams) -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    loop {
        {
            let state = shared.lock().unwrap();
            if !state.running {
                break;
            }
        }

        terminal.draw(|f| {
            let state = shared.lock().unwrap();
            draw_ui(f, &state);
        })?;

        if event::poll(Duration::from_millis(16))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                let mut state = shared.lock().unwrap();
                if state.values.is_empty() {
                    if key.code == KeyCode::Char('q') || key.code == KeyCode::Esc {
                        state.running = false;
                        break;
                    }
                    continue;
                }

                let coarse = key.modifiers.contains(KeyModifiers::SHIFT);
                let factor = if coarse { 1.2 } else { 1.05 };

                match key.code {
                    KeyCode::Up => {
                        if state.selected > 0 {
                            state.selected -= 1;
                        }
                    }
                    KeyCode::Down => {
                        if state.selected + 1 < state.values.len() {
                            state.selected += 1;
                        }
                    }
                    KeyCode::Left => {
                        let i = state.selected;
                        state.values[i] /= factor as f32;
                    }
                    KeyCode::Right => {
                        let i = state.selected;
                        state.values[i] *= factor as f32;
                    }
                    KeyCode::Char('d') => {
                        let i = state.selected;
                        state.values[i] = state.defaults[i];
                    }
                    KeyCode::Char('s') => {
                        match save_params(&state) {
                            Ok(path) => eprintln!("Saved to {}", path),
                            Err(e) => eprintln!("Save failed: {}", e),
                        }
                    }
                    KeyCode::Char('q') | KeyCode::Esc => {
                        state.running = false;
                        break;
                    }
                    _ => {}
                }
            }
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    Ok(())
}

fn draw_ui(f: &mut ratatui::Frame, state: &ParamState) {
    let area = f.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),
            Constraint::Length(3),
        ])
        .split(area);

    // Parameter list
    let param_count = state.values.len();
    let row_constraints: Vec<Constraint> = (0..param_count)
        .map(|_| Constraint::Length(2))
        .collect();

    let param_area = chunks[0];
    let param_block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" {} — Parameters ", state.title));
    let inner = param_block.inner(param_area);
    f.render_widget(param_block, param_area);

    if param_count == 0 {
        let msg = Paragraph::new("No tunable parameters");
        f.render_widget(msg, inner);
        return;
    }

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints(row_constraints)
        .split(inner);

    let max_name_len = state.names.iter().map(|n| n.len()).max().unwrap_or(0);

    for (i, name) in state.names.iter().enumerate() {
        let val = state.values[i];
        let default = state.defaults[i];
        let is_selected = i == state.selected;

        let row_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length((max_name_len + 2) as u16),
                Constraint::Length(14),
                Constraint::Min(10),
            ])
            .split(rows[i]);

        // Name
        let name_style = if is_selected {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        let indicator = if is_selected { "▸ " } else { "  " };
        let name_text = Paragraph::new(format!("{}{}", indicator, name)).style(name_style);
        f.render_widget(name_text, row_chunks[0]);

        // Value
        let val_style = if is_selected {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::Gray)
        };
        let val_text = Paragraph::new(format!("{:.6}", val)).style(val_style);
        f.render_widget(val_text, row_chunks[1]);

        // Bar — log scale relative to default (center = default)
        let ratio = if default > 0.0 && val > 0.0 {
            let log_ratio = (val / default).ln() / (4.0_f32).ln();
            ((log_ratio + 1.0) / 2.0).clamp(0.0, 1.0)
        } else {
            0.5
        };

        let bar_style = if is_selected {
            Style::default().fg(Color::Yellow).bg(Color::DarkGray)
        } else {
            Style::default().fg(Color::Blue).bg(Color::DarkGray)
        };
        let gauge = Gauge::default()
            .gauge_style(bar_style)
            .ratio(ratio as f64)
            .label("");
        f.render_widget(gauge, row_chunks[2]);
    }

    // Footer
    let help = Line::from(vec![
        Span::styled("↑↓", Style::default().fg(Color::Yellow)),
        Span::raw(" select  "),
        Span::styled("←→", Style::default().fg(Color::Yellow)),
        Span::raw(" adjust  "),
        Span::styled("shift+←→", Style::default().fg(Color::Yellow)),
        Span::raw(" coarse  "),
        Span::styled("d", Style::default().fg(Color::Yellow)),
        Span::raw(" reset  "),
        Span::styled("q", Style::default().fg(Color::Yellow)),
        Span::raw(" quit"),
    ]);
    let footer = Paragraph::new(help)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(footer, chunks[1]);
}
