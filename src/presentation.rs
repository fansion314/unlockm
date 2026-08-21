use clap::ValueEnum;
use std::{
    io::{IsTerminal, Write},
    path::Path,
    sync::{Arc, Mutex},
    time::Instant,
};
use tracing_subscriber::fmt::MakeWriter;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, ValueEnum)]
pub enum ProgressMode {
    #[default]
    Auto,
    Always,
    Never,
}

impl std::fmt::Display for ProgressMode {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Auto => "auto",
            Self::Always => "always",
            Self::Never => "never",
        })
    }
}

#[derive(Clone)]
pub struct ProgressReporter(Arc<Inner>);

struct Inner {
    enabled: bool,
    state: Mutex<State>,
}

struct State {
    mode: StateMode,
    total: usize,
    completed: usize,
    active: usize,
    latest: String,
    phase: &'static str,
    started: Instant,
    drawn: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum StateMode {
    Idle,
    Batch,
    Watch,
}

impl std::fmt::Debug for ProgressReporter {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ProgressReporter")
            .field("enabled", &self.0.enabled)
            .finish_non_exhaustive()
    }
}

impl Default for ProgressReporter {
    fn default() -> Self {
        Self::disabled()
    }
}

impl ProgressReporter {
    pub fn new(mode: ProgressMode, json_logs: bool) -> Self {
        let terminal = std::io::stderr().is_terminal();
        let dumb = std::env::var("TERM").is_ok_and(|term| term == "dumb");
        let no_color = std::env::var_os("NO_COLOR").is_some();
        let enabled = progress_enabled(mode, json_logs, terminal, dumb, no_color);
        Self(Arc::new(Inner {
            enabled,
            state: Mutex::new(State {
                mode: StateMode::Idle,
                total: 0,
                completed: 0,
                active: 0,
                latest: String::new(),
                phase: "Waiting",
                started: Instant::now(),
                drawn: false,
            }),
        }))
    }

    pub fn disabled() -> Self {
        Self::new(ProgressMode::Never, false)
    }

    pub fn is_interactive(&self) -> bool {
        self.0.enabled
    }

    pub fn begin_batch(&self, total: usize) {
        if !self.0.enabled {
            return;
        }
        let mut state = self.0.state.lock().expect("progress state");
        state.mode = StateMode::Batch;
        state.total = total;
        state.completed = 0;
        state.active = 0;
        state.latest.clear();
        state.phase = "Planning";
        state.started = Instant::now();
        redraw(&mut state);
    }

    pub fn begin_watch(&self) {
        if !self.0.enabled {
            return;
        }
        let mut state = self.0.state.lock().expect("progress state");
        state.mode = StateMode::Watch;
        state.active = 0;
        state.latest.clear();
        state.phase = "Waiting";
        redraw(&mut state);
    }

    pub fn activity(&self, path: &Path, phase: &'static str) {
        if !self.0.enabled {
            return;
        }
        let mut state = self.0.state.lock().expect("progress state");
        state.latest = path.display().to_string();
        state.phase = phase;
        redraw(&mut state);
    }

    pub fn started(&self, path: &Path, phase: &'static str) {
        if !self.0.enabled {
            return;
        }
        let mut state = self.0.state.lock().expect("progress state");
        state.active += 1;
        state.latest = path.display().to_string();
        state.phase = phase;
        redraw(&mut state);
    }

    pub fn finished(&self) {
        if !self.0.enabled {
            return;
        }
        let mut state = self.0.state.lock().expect("progress state");
        state.completed += 1;
        state.active = state.active.saturating_sub(1);
        if state.active == 0 {
            state.phase = if state.mode == StateMode::Watch {
                "Waiting"
            } else {
                "Finishing"
            };
        }
        redraw(&mut state);
    }

    pub fn finish(&self) {
        if !self.0.enabled {
            return;
        }
        let mut state = self.0.state.lock().expect("progress state");
        let was_active = state.drawn || state.mode != StateMode::Idle;
        clear(&mut state);
        state.mode = StateMode::Idle;
        if was_active {
            let _ = std::io::stderr().write_all(b"\x1b[?25h");
            let _ = std::io::stderr().flush();
        }
    }

    pub fn make_writer(&self) -> ProgressMakeWriter {
        ProgressMakeWriter(self.clone())
    }

    fn write_log(&self, bytes: &[u8]) {
        let mut state = self.0.state.lock().expect("progress state");
        if self.0.enabled {
            clear(&mut state);
        }
        let mut stderr = std::io::stderr().lock();
        let _ = stderr.write_all(bytes);
        if !bytes.ends_with(b"\n") {
            let _ = stderr.write_all(b"\n");
        }
        let _ = stderr.flush();
        if self.0.enabled && state.mode != StateMode::Idle {
            redraw(&mut state);
        }
    }
}

fn progress_enabled(
    mode: ProgressMode,
    json_logs: bool,
    terminal: bool,
    dumb: bool,
    no_color: bool,
) -> bool {
    !json_logs
        && match mode {
            ProgressMode::Auto => terminal && !dumb && !no_color,
            ProgressMode::Always => true,
            ProgressMode::Never => false,
        }
}

#[derive(Clone)]
pub struct ProgressMakeWriter(ProgressReporter);

pub struct ProgressWriter {
    reporter: ProgressReporter,
    buffer: Vec<u8>,
}

impl<'writer> MakeWriter<'writer> for ProgressMakeWriter {
    type Writer = ProgressWriter;

    fn make_writer(&'writer self) -> Self::Writer {
        ProgressWriter {
            reporter: self.0.clone(),
            buffer: Vec::new(),
        }
    }
}

impl Write for ProgressWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.buffer.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        if !self.buffer.is_empty() {
            self.reporter.write_log(&self.buffer);
            self.buffer.clear();
        }
        Ok(())
    }
}

impl Drop for ProgressWriter {
    fn drop(&mut self) {
        let _ = self.flush();
    }
}

fn redraw(state: &mut State) {
    if state.drawn {
        clear(state);
    } else {
        let _ = std::io::stderr().write_all(b"\x1b[?25l");
    }
    let (top, bottom) = lines(state);
    let mut stderr = std::io::stderr().lock();
    let _ = write!(stderr, "\r\x1b[2K{top}\n\r\x1b[2K{bottom}\x1b[1A");
    let _ = stderr.flush();
    state.drawn = true;
}

fn clear(state: &mut State) {
    if !state.drawn {
        return;
    }
    let mut stderr = std::io::stderr().lock();
    let _ = stderr.write_all(b"\r\x1b[2K\n\r\x1b[2K\x1b[1A\r");
    let _ = stderr.flush();
    state.drawn = false;
}

fn lines(state: &State) -> (String, String) {
    let elapsed = state.started.elapsed().as_secs_f32();
    let top = match state.mode {
        StateMode::Batch => {
            let fraction = if state.total == 0 {
                1.0
            } else {
                state.completed.min(state.total) as f32 / state.total as f32
            };
            let width = terminal_width().saturating_sub(24).clamp(8, 24);
            let filled = (fraction * width as f32).round() as usize;
            let bar = format!("{}{}", "=".repeat(filled), "-".repeat(width - filled));
            format!(
                "[{bar}] {}/{} {:>5.1}%  {:.1}s",
                state.completed.min(state.total),
                state.total,
                fraction * 100.0,
                elapsed
            )
        }
        StateMode::Watch => format!(
            "Watching  processed={} active={}  {:.1}s",
            state.completed, state.active, elapsed
        ),
        StateMode::Idle => String::new(),
    };
    let active = state.active.saturating_sub(1);
    let suffix = if active == 0 {
        String::new()
    } else {
        format!(" (+{active} active)")
    };
    let available = terminal_width().saturating_sub(state.phase.len() + suffix.len() + 2);
    let path = truncate_middle(&state.latest, available.max(8));
    let bottom = if path.is_empty() {
        state.phase.to_owned()
    } else {
        format!("{}: {path}{suffix}", state.phase)
    };
    (top, bottom)
}

fn terminal_width() -> usize {
    std::env::var("COLUMNS")
        .ok()
        .and_then(|value| value.parse().ok())
        .filter(|width| *width >= 20)
        .unwrap_or(80)
}

fn truncate_middle(value: &str, width: usize) -> String {
    let characters = value.chars().collect::<Vec<_>>();
    if characters.len() <= width {
        return value.to_owned();
    }
    if width <= 3 {
        return "…".into();
    }
    let left = (width - 1) / 2;
    let right = width - 1 - left;
    format!(
        "{}…{}",
        characters[..left].iter().collect::<String>(),
        characters[characters.len() - right..]
            .iter()
            .collect::<String>()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unicode_paths_truncate_without_splitting_characters() {
        let value = truncate_middle("目录/とても長い曲名/歌曲.ncm", 12);
        assert!(value.contains('…'));
        assert!(value.is_char_boundary(value.len()));
        assert!(value.chars().count() <= 12);
    }

    #[test]
    fn never_mode_is_non_interactive() {
        assert!(!ProgressReporter::new(ProgressMode::Never, false).is_interactive());
        assert!(!ProgressReporter::new(ProgressMode::Always, true).is_interactive());
    }

    #[test]
    fn auto_honors_tty_dumb_terminal_no_color_and_json_policy() {
        assert!(progress_enabled(
            ProgressMode::Auto,
            false,
            true,
            false,
            false
        ));
        assert!(!progress_enabled(
            ProgressMode::Auto,
            false,
            false,
            false,
            false
        ));
        assert!(!progress_enabled(
            ProgressMode::Auto,
            false,
            true,
            true,
            false
        ));
        assert!(!progress_enabled(
            ProgressMode::Auto,
            false,
            true,
            false,
            true
        ));
        assert!(!progress_enabled(
            ProgressMode::Auto,
            true,
            true,
            false,
            false
        ));
        assert!(progress_enabled(
            ProgressMode::Always,
            false,
            false,
            true,
            true
        ));
    }

    #[test]
    fn watch_status_has_no_percentage_and_reports_latest_active_path() {
        let state = State {
            mode: StateMode::Watch,
            total: 10,
            completed: 12,
            active: 3,
            latest: "album/song.ncm".into(),
            phase: "Decrypting",
            started: Instant::now(),
            drawn: false,
        };
        let (top, bottom) = lines(&state);
        assert!(top.starts_with("Watching  processed=12 active=3"));
        assert!(!top.contains('%'));
        assert!(bottom.contains("Decrypting: album/song.ncm (+2 active)"));
    }
}
