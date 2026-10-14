//! Terminal emulation on the embedder's side (`spawn {grid: true}` on `silica/pty`): the
//! pty output is parsed here with `alacritty_terminal`, and the app only receives the
//! visible rows that changed, once per frame, as runs of text with the same attributes.
//! Parsing runs on a thread of its own ([`GridWorker`]): neither the app's UI thread nor the
//! embedder's main thread parses escape sequences, so a flood of output cannot starve frames
//! or input.
//!
//! `grid {id, cols, rows, full, lines: [{y, runs: [[text, fg, bg, flags], …]}],
//! cursor: {x, y, visible, shape}, modes, title?}` — colours are 0xRRGGBB, -1 for the
//! theme's default foreground or background; `flags` bits: 1 bold, 2 italic, 4 underline,
//! 8 inverse, 16 dim, 32 strikeout, 64 curly underline, 128 hidden, 256 wide.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, mpsc};

use alacritty_terminal::event::{Event, EventListener};
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::color::Colors;
use alacritty_terminal::term::{Config, Term, TermDamage, TermMode, test::TermSize};
use alacritty_terminal::vte::ansi::{Color, CursorShape, NamedColor, Processor, Rgb};
use serde_json::{Value, json};

/// Lines kept above the screen.
const SCROLLBACK: usize = 10_000;

#[derive(Clone, Default)]
struct Listener {
    events: Arc<Mutex<Vec<Event>>>,
}

impl EventListener for Listener {
    fn send_event(&self, event: Event) {
        if let Ok(mut events) = self.events.lock() {
            events.push(event);
        }
    }
}

/// Bytes parsed per lock of the grid: the frame can take the lock between slices.
const SLICE: usize = 16 * 1024;

/// A [`TermGrid`] fed by a parser thread.
pub struct GridWorker {
    grid: Arc<Mutex<TermGrid>>,
    input: mpsc::Sender<Vec<u8>>,
    queued: Arc<AtomicUsize>,
    /// The main thread wants the grid: the parser lets it in between slices (a std Mutex is
    /// not fair, and the parser would otherwise take it back at once).
    waiting: Arc<AtomicUsize>,
}

impl GridWorker {
    pub fn new(cols: u16, rows: u16) -> std::io::Result<Self> {
        let grid = Arc::new(Mutex::new(TermGrid::new(cols, rows)));
        let queued = Arc::new(AtomicUsize::new(0));
        let (input, chunks) = mpsc::channel::<Vec<u8>>();
        let waiting = Arc::new(AtomicUsize::new(0));
        let (shared, count, wait) = (grid.clone(), queued.clone(), waiting.clone());
        std::thread::Builder::new()
            .name("silica-term".to_owned())
            .spawn(move || {
                // Ends when the session drops its sender.
                for chunk in chunks {
                    for slice in chunk.chunks(SLICE) {
                        while wait.load(Ordering::Acquire) > 0 {
                            std::thread::yield_now();
                        }
                        if let Ok(mut grid) = shared.lock() {
                            grid.advance(slice);
                        }
                    }
                    count.fetch_sub(chunk.len(), Ordering::Relaxed);
                }
            })?;
        Ok(Self {
            grid,
            input,
            queued,
            waiting,
        })
    }

    /// Hands output to the parser thread.
    pub fn feed(&self, bytes: Vec<u8>) {
        let len = bytes.len();
        self.queued.fetch_add(len, Ordering::Relaxed);
        if self.input.send(bytes).is_err() {
            self.queued.fetch_sub(len, Ordering::Relaxed);
        }
    }

    /// Bytes handed over and not parsed yet.
    pub fn queued(&self) -> usize {
        self.queued.load(Ordering::Relaxed)
    }

    pub fn resize(&self, cols: u16, rows: u16) {
        self.waiting.fetch_add(1, Ordering::AcqRel);
        if let Ok(mut grid) = self.grid.lock() {
            grid.resize(cols, rows);
        }
        self.waiting.fetch_sub(1, Ordering::AcqRel);
    }

    /// The frame message (if anything changed) and the replies for the program.
    pub fn frame(&self, id: u32) -> (Option<Value>, Vec<u8>) {
        self.waiting.fetch_add(1, Ordering::AcqRel);
        let result = match self.grid.lock() {
            Ok(mut grid) => {
                let replies = grid.take_replies();
                (grid.frame(id), replies)
            }
            Err(_) => (None, Vec::new()),
        };
        self.waiting.fetch_sub(1, Ordering::AcqRel);
        result
    }
}

pub struct TermGrid {
    term: Term<Listener>,
    parser: Processor,
    events: Arc<Mutex<Vec<Event>>>,
    /// Bytes the terminal answers to the program (device attributes, cursor position…).
    replies: Vec<u8>,
    title: Option<String>,
    full: bool,
    /// The rows as last sent, to skip the ones that did not really change.
    sent: Vec<Value>,
    cursor: Value,
    modes: u32,
}

impl TermGrid {
    pub fn new(cols: u16, rows: u16) -> Self {
        let listener = Listener::default();
        let events = listener.events.clone();
        let config = Config {
            scrolling_history: SCROLLBACK,
            ..Config::default()
        };
        let size = TermSize::new(usize::from(cols.max(1)), usize::from(rows.max(1)));
        Self {
            term: Term::new(config, &size, listener),
            parser: Processor::new(),
            events,
            replies: Vec::new(),
            title: None,
            full: true,
            sent: Vec::new(),
            cursor: Value::Null,
            modes: 0,
        }
    }

    pub fn advance(&mut self, bytes: &[u8]) {
        self.parser.advance(&mut self.term, bytes);
        let events: Vec<Event> = match self.events.lock() {
            Ok(mut events) => events.drain(..).collect(),
            Err(_) => Vec::new(),
        };
        for event in events {
            match event {
                Event::PtyWrite(text) => self.replies.extend_from_slice(text.as_bytes()),
                Event::Title(title) => self.title = Some(title),
                Event::ResetTitle => self.title = Some(String::new()),
                _ => {}
            }
        }
    }

    /// What the terminal must write back to the program.
    pub fn take_replies(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.replies)
    }

    pub fn resize(&mut self, cols: u16, rows: u16) {
        let size = TermSize::new(usize::from(cols.max(1)), usize::from(rows.max(1)));
        self.term.resize(size);
        self.full = true;
    }

    /// The `grid` message for the rows changed since the last call; None if nothing changed.
    pub fn frame(&mut self, id: u32) -> Option<Value> {
        let rows = self.term.screen_lines();
        let cols = self.term.columns();
        let full = std::mem::take(&mut self.full);
        let damaged: Vec<usize> = match (full, self.term.damage()) {
            (true, _) | (_, TermDamage::Full) => (0..rows).collect(),
            (false, TermDamage::Partial(lines)) => lines.map(|l| l.line).collect(),
        };
        if full {
            self.sent.clear();
        }
        self.sent.resize(rows, Value::Null);
        let colors = *self.term.colors();
        let grid = self.term.grid();
        let offset = grid.display_offset();
        let mut lines = Vec::new();
        for y in damaged.into_iter().filter(|&y| y < rows) {
            let row = &grid[Line(y as i32 - offset as i32)];
            let runs = row_runs((0..cols).map(|x| &row[Column(x)]), &colors);
            // alacritty marks the cursor's rows damaged on every frame: send real changes only.
            if self.sent.get(y) != Some(&runs) {
                lines.push(json!({ "y": y, "runs": runs.clone() }));
                if let Some(slot) = self.sent.get_mut(y) {
                    *slot = runs;
                }
            }
        }
        self.term.reset_damage();
        let cursor = self.term.renderable_content().cursor;
        let state = json!({
            "x": cursor.point.column.0,
            "y": cursor.point.line.0 + offset as i32,
            "visible": cursor.shape != CursorShape::Hidden,
            "shape": match cursor.shape {
                CursorShape::Beam => "beam",
                CursorShape::Underline => "underline",
                _ => "block",
            },
        });
        let modes = mode_bits(*self.term.mode());
        let title = self.title.take();
        let moved = self.cursor != state || self.modes != modes;
        if lines.is_empty() && !moved && title.is_none() && !full {
            return None;
        }
        self.cursor = state.clone();
        self.modes = modes;
        let mut message = json!({
            "id": id,
            "cols": cols,
            "rows": rows,
            "full": full,
            "lines": lines,
            "cursor": state,
            "modes": modes,
        });
        if let (Some(title), Some(object)) = (title, message.as_object_mut()) {
            object.insert("title".to_owned(), Value::String(title));
        }
        Some(message)
    }
}

/// The runs of a row: `[text, fg, bg, flags]` for each stretch of cells with the same
/// attributes, without the default blanks at the end.
fn row_runs<'a>(
    cells: impl Iterator<Item = &'a alacritty_terminal::term::cell::Cell>,
    colors: &Colors,
) -> Value {
    let mut runs: Vec<Value> = Vec::new();
    let mut text = String::new();
    let mut key: Option<(i64, i64, u32)> = None;
    for cell in cells {
        if cell.flags.contains(Flags::WIDE_CHAR_SPACER) {
            continue;
        }
        let attrs = (
            resolve(cell.fg, colors, true),
            resolve(cell.bg, colors, false),
            flag_bits(cell.flags),
        );
        if let Some((fg, bg, flags)) = key.filter(|&k| k != attrs) {
            runs.push(json!([std::mem::take(&mut text), fg, bg, flags]));
        }
        key = Some(attrs);
        text.push(if cell.c == '\0' { ' ' } else { cell.c });
        if let Some(zw) = cell.zerowidth() {
            text.extend(zw);
        }
    }
    if let Some((fg, bg, flags)) = key {
        if fg == -1 && bg == -1 && flags == 0 {
            let trimmed = text.trim_end_matches(' ');
            if !trimmed.is_empty() {
                runs.push(json!([trimmed, fg, bg, flags]));
            }
        } else {
            runs.push(json!([text, fg, bg, flags]));
        }
    }
    Value::Array(runs)
}

/// Mode bits the app needs to encode input: 1 application cursor keys, 2 bracketed paste,
/// 4 mouse reporting (any), 8 SGR mouse, 16 focus events, 32 alternate screen, 64 app keypad.
fn mode_bits(mode: TermMode) -> u32 {
    let mut bits = 0;
    if mode.contains(TermMode::APP_CURSOR) {
        bits |= 1;
    }
    if mode.contains(TermMode::BRACKETED_PASTE) {
        bits |= 2;
    }
    if mode.intersects(TermMode::MOUSE_MODE) {
        bits |= 4;
    }
    if mode.contains(TermMode::SGR_MOUSE) {
        bits |= 8;
    }
    if mode.contains(TermMode::FOCUS_IN_OUT) {
        bits |= 16;
    }
    if mode.contains(TermMode::ALT_SCREEN) {
        bits |= 32;
    }
    if mode.contains(TermMode::APP_KEYPAD) {
        bits |= 64;
    }
    bits
}

fn flag_bits(flags: Flags) -> u32 {
    let mut bits = 0;
    for (flag, bit) in [
        (Flags::BOLD, 1),
        (Flags::ITALIC, 2),
        (Flags::UNDERLINE, 4),
        (Flags::INVERSE, 8),
        (Flags::DIM, 16),
        (Flags::STRIKEOUT, 32),
        (Flags::UNDERCURL, 64),
        (Flags::HIDDEN, 128),
        (Flags::WIDE_CHAR, 256),
    ] {
        if flags.contains(flag) {
            bits |= bit;
        }
    }
    bits
}

fn rgb(c: Rgb) -> i64 {
    (i64::from(c.r) << 16) | (i64::from(c.g) << 8) | i64::from(c.b)
}

/// A cell colour as 0xRRGGBB, or -1 for the theme's default (foreground or background).
fn resolve(color: Color, colors: &Colors, foreground: bool) -> i64 {
    match color {
        Color::Spec(c) => rgb(c),
        Color::Named(NamedColor::Foreground) if foreground => -1,
        Color::Named(NamedColor::Background) if !foreground => -1,
        Color::Named(NamedColor::Foreground | NamedColor::Background) => -1,
        Color::Named(name) => indexed(name as usize, colors),
        Color::Indexed(i) => indexed(usize::from(i), colors),
    }
}

/// The xterm palette (overridable by OSC 4 through `colors`).
fn indexed(index: usize, colors: &Colors) -> i64 {
    if let Some(Some(c)) = (index < 256).then(|| colors[index]) {
        return rgb(c);
    }
    const BASE: [i64; 16] = [
        0x000000, 0xcd0000, 0x00cd00, 0xcdcd00, 0x0000ee, 0xcd00cd, 0x00cdcd, 0xe5e5e5, 0x7f7f7f,
        0xff0000, 0x00ff00, 0xffff00, 0x5c5cff, 0xff00ff, 0x00ffff, 0xffffff,
    ];
    match index {
        0..=15 => BASE.get(index).copied().unwrap_or(0),
        16..=231 => {
            let i = index - 16;
            let level = |v: usize| if v == 0 { 0 } else { 55 + v as i64 * 40 };
            (level(i / 36) << 16) | (level(i / 6 % 6) << 8) | level(i % 6)
        }
        232..=255 => {
            let v = 8 + (index as i64 - 232) * 10;
            (v << 16) | (v << 8) | v
        }
        // Dim and bright variants of the named colours: the plain ones.
        256..=268 => -1,
        _ => -1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn runs(message: &Value, y: usize) -> Vec<Value> {
        message["lines"]
            .as_array()
            .and_then(|lines| lines.iter().find(|l| l["y"] == y))
            .and_then(|l| l["runs"].as_array())
            .cloned()
            .unwrap_or_default()
    }

    #[test]
    fn text_and_colours_become_runs() {
        let mut grid = TermGrid::new(20, 4);
        grid.advance(b"ab\x1b[31mcd\x1b[0m\r\n\x1b[48;2;1;2;3mX");
        let message = grid.frame(7).unwrap();
        assert_eq!(message["id"], 7);
        assert_eq!(message["full"], true);
        let first = runs(&message, 0);
        assert_eq!(first[0], json!(["ab", -1, -1, 0]));
        assert_eq!(first[1], json!(["cd", 0xcd0000, -1, 0]));
        assert_eq!(runs(&message, 1)[0], json!(["X", -1, 0x010203, 0]));
        assert_eq!(message["cursor"]["y"], 1);
        // Nothing new: nothing to send.
        assert!(grid.frame(7).is_none());
        // Only the changed row next time.
        grid.advance(b"!");
        let message = grid.frame(7).unwrap();
        assert_eq!(message["full"], false);
        assert_eq!(message["lines"].as_array().map(Vec::len), Some(1));
        assert!(grid.frame(7).is_none());
    }

    #[test]
    fn device_attributes_are_answered_and_titles_reported() {
        let mut grid = TermGrid::new(10, 2);
        grid.advance(b"\x1b[c\x1b]0;hello\x07");
        assert!(!grid.take_replies().is_empty());
        assert_eq!(grid.frame(1).unwrap()["title"], "hello");
    }

    #[test]
    fn modes_follow_the_program() {
        let mut grid = TermGrid::new(10, 2);
        grid.advance(b"\x1b[?1h\x1b[?2004h\x1b[?1049h");
        let modes = grid.frame(1).unwrap()["modes"].as_u64().unwrap_or(0);
        assert_eq!(modes & (1 | 2 | 32), 1 | 2 | 32);
    }

    #[test]
    fn palette() {
        let colors = Colors::default();
        assert_eq!(indexed(1, &colors), 0xcd0000);
        assert_eq!(indexed(196, &colors), 0xff0000);
        assert_eq!(indexed(232, &colors), 0x080808);
    }
}
