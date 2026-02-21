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

#[derive(Clone)]
pub struct ParamChange {
    pub tick: u32,
    pub param: String,
    pub value: f32,
}

pub struct ParamState {
    pub values: Vec<f32>,
    pub defaults: Vec<f32>,
    pub names: Vec<String>,
    pub selected: usize,
    pub title: String,
    pub running: bool,
    pub tick: u32,
    pub history: Vec<ParamChange>,
    pub replay: Vec<ParamChange>,
    pub replay_cursor: usize,
}

impl ParamState {
    pub fn set_param(&mut self, idx: usize, value: f32) {
        self.values[idx] = value;
        if self.replay.is_empty() {
            self.history.push(ParamChange {
                tick: self.tick,
                param: self.names[idx].clone(),
                value,
            });
        }
    }

    pub fn apply_pending_replay(&mut self) {
        while self.replay_cursor < self.replay.len()
            && self.replay[self.replay_cursor].tick <= self.tick
        {
            let change = self.replay[self.replay_cursor].clone();
            if let Some(idx) = self.names.iter().position(|n| n == &change.param) {
                self.values[idx] = change.value;
            }
            self.replay_cursor += 1;
        }
    }

    pub fn clear_history(&mut self) {
        self.history.clear();
        self.replay.clear();
        self.replay_cursor = 0;
    }

    pub fn is_replaying(&self) -> bool {
        !self.replay.is_empty()
    }
}

pub type SharedParams = Arc<Mutex<ParamState>>;

pub fn save_params(state: &ParamState) -> io::Result<String> {
    let mut initial_entries: Vec<String> = Vec::new();
    for (name, &val) in state.names.iter().zip(state.defaults.iter()) {
        initial_entries.push(format!("    \"{}\": {}", name, val));
    }
    let initial = format!("{{\n{}\n  }}", initial_entries.join(",\n"));

    let history_entries: Vec<String> = state.history.iter().map(|c| {
        format!("    {{\"tick\": {}, \"param\": \"{}\", \"value\": {}}}", c.tick, c.param, c.value)
    }).collect();
    let history = format!("[\n{}\n  ]", history_entries.join(",\n"));

    let json = format!("{{\n  \"initial\": {},\n  \"history\": {}\n}}", initial, history);

    let filename = format!("{}_params.json", state.title.to_lowercase().replace(' ', "_"));
    std::fs::write(&filename, &json)?;
    Ok(filename)
}

pub fn load_params(state: &mut ParamState, path: &Path) -> io::Result<()> {
    let contents = std::fs::read_to_string(path)?;
    let parsed: serde_json::Value = serde_json::from_str(&contents)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    if let serde_json::Value::Object(ref map) = parsed {
        // New format with initial + history
        if let Some(serde_json::Value::Object(initial)) = map.get("initial") {
            for (name, val) in initial {
                if let Some(idx) = state.names.iter().position(|n| n == name) {
                    if let Some(v) = val.as_f64() {
                        state.values[idx] = v as f32;
                    }
                }
            }

            if let Some(serde_json::Value::Array(history)) = map.get("history") {
                state.replay.clear();
                state.replay_cursor = 0;
                for entry in history {
                    if let (Some(tick), Some(param), Some(value)) = (
                        entry.get("tick").and_then(|t| t.as_u64()),
                        entry.get("param").and_then(|p| p.as_str()),
                        entry.get("value").and_then(|v| v.as_f64()),
                    ) {
                        state.replay.push(ParamChange {
                            tick: tick as u32,
                            param: param.to_string(),
                            value: value as f32,
                        });
                    }
                }
            }
        } else {
            // Legacy flat format: {"NAME": value, ...}
            for (name, val) in map {
                if let Some(idx) = state.names.iter().position(|n| n == name) {
                    if let Some(v) = val.as_f64() {
                        state.values[idx] = v as f32;
                    }
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

                let fine = key.modifiers.contains(KeyModifiers::SHIFT);
                let factor = if fine { 1.00625 } else { 1.05 };

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
                        let new_val = state.values[i] / factor as f32;
                        state.set_param(i, new_val);
                    }
                    KeyCode::Right => {
                        let i = state.selected;
                        let new_val = state.values[i] * factor as f32;
                        state.set_param(i, new_val);
                    }
                    KeyCode::Char('d') => {
                        let i = state.selected;
                        let default = state.defaults[i];
                        state.set_param(i, default);
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
    let title = if state.is_replaying() {
        format!(" {} — Parameters [REPLAY] ", state.title)
    } else {
        format!(" {} — Parameters ", state.title)
    };
    let param_block = Block::default()
        .borders(Borders::ALL)
        .title(title);
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
    let change_count = state.history.len();
    let mut help_spans = vec![
        Span::styled("↑↓", Style::default().fg(Color::Yellow)),
        Span::raw(" select  "),
        Span::styled("←→", Style::default().fg(Color::Yellow)),
        Span::raw(" adjust  "),
        Span::styled("shift+←→", Style::default().fg(Color::Yellow)),
        Span::raw(" fine  "),
        Span::styled("d", Style::default().fg(Color::Yellow)),
        Span::raw(" reset  "),
        Span::styled("s", Style::default().fg(Color::Yellow)),
        Span::raw(" save  "),
        Span::styled("q", Style::default().fg(Color::Yellow)),
        Span::raw(" quit"),
    ];
    if change_count > 0 {
        help_spans.push(Span::raw("  "));
        help_spans.push(Span::styled(
            format!("[{} changes]", change_count),
            Style::default().fg(Color::DarkGray),
        ));
    }
    let help = Line::from(help_spans);
    let footer = Paragraph::new(help)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(footer, chunks[1]);
}
