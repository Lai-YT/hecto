use std::env;
use std::time::{Duration, Instant};

use crate::Document;
use crate::Row;
use crate::Terminal;
use std::io::Error;
use termion::color;
use termion::event::Key;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const STATUS_BG_COLOR: color::Rgb = color::Rgb(239, 239, 239);
const STATUS_FG_COLOR: color::Rgb = color::Rgb(63, 63, 63);
/// The number of times the user has to press `Ctrl-Q` to quit.
const QUIT_TIMES: u8 = 3;

#[derive(Default)]
pub struct Position {
    pub x: usize,
    pub y: usize,
}

struct StatusMessage {
    text: String,
    time: Instant,
}

impl StatusMessage {
    fn from(text: String) -> Self {
        Self {
            text,
            time: Instant::now(),
        }
    }

    /// A shorthand for `StatusMessage::from(String::new())`.
    fn clear(&mut self) {
        self.text.clear();
        self.time = Instant::now();
    }
}

pub struct Editor {
    should_quit: bool,
    terminal: Terminal,
    document: Document,
    /// Where of the file the user is currently scrolled to.
    offset: Position,
    cursor_position: Position,
    status_message: StatusMessage,
    quit_times: u8,
}

impl Default for Editor {
    fn default() -> Self {
        let args: Vec<String> = env::args().collect();
        let mut initial_status = String::from("HELP: Ctrl-S = save | Ctrl-Q = quit");
        let document = if let Some(filename) = args.get(1) {
            if let Ok(doc) = Document::open(filename) {
                doc
            } else {
                initial_status = format!("ERR: Could not open file: {filename}");
                Document::default()
            }
        } else {
            Document::default()
        };
        Self {
            should_quit: false,
            #[allow(clippy::expect_used)]
            terminal: Terminal::new().expect("Failed to initialize terminal"),
            document,
            offset: Position::default(),
            // top-left corner
            cursor_position: Position::default(),
            status_message: StatusMessage::from(initial_status),
            quit_times: QUIT_TIMES,
        }
    }
}

impl Editor {
    pub fn run(&mut self) {
        loop {
            // NOTE: The screen is refreshed before quitting.
            if let Err(e) = &self.refresh_screen() {
                die(e);
            }
            if self.should_quit {
                break;
            }
            if let Err(e) = &self.process_keypress() {
                die(e);
            }
        }
    }

    fn refresh_screen(&self) -> Result<(), Error> {
        Terminal::cursor_hide(); // prevent the cursor from blinking
        Terminal::cursor_position(&Position::default());
        if self.should_quit {
            Terminal::clear_screen();
            println!("Goodbye.\r");
        } else {
            self.draw_rows();
            self.draw_status_bar();
            self.draw_message_bar();
            let cursor_pos_relative_to_offset = Position {
                x: self.cursor_position.x.saturating_sub(self.offset.x),
                y: self.cursor_position.y.saturating_sub(self.offset.y),
            };
            Terminal::cursor_position(&cursor_pos_relative_to_offset);
        }
        Terminal::cursor_show();
        Terminal::flush()
    }

    /// If the row exists, draw it.
    /// Otherwise, draw a tilde, meaning that row is not part of the document and
    /// can't contain any text.
    fn draw_rows(&self) {
        let height = self.terminal.size().height;
        // The last line is kept empty for the status bar.
        for term_row in 0..height {
            Terminal::clear_current_line();
            // If such row exists, draw it.
            #[allow(clippy::integer_division)]
            if let Some(row) = self
                .document
                .row(self.offset.y.saturating_add(term_row as usize))
            {
                self.draw_row(row);
            } else if self.document.is_empty() && term_row == height / 3 {
                // XXX: Should we draw the welcome message if we do open an empty file?
                self.draw_welcome_message();
            } else {
                println!("~\r");
            }
        }
    }

    fn draw_welcome_message(&self) {
        let mut welcome_msg = format!("Hecto editor -- version {VERSION}");
        let term_width = self.terminal.size().width as usize;
        let msg_len = welcome_msg.len();
        // The padding is the number of spaces to add to the left of the message.
        #[allow(clippy::integer_division)]
        let padding = term_width.saturating_sub(msg_len) / 2;
        let spaces = " ".repeat(padding.saturating_add(1 /* for ~ */));
        welcome_msg = format!("~{spaces}{welcome_msg}\r");
        welcome_msg.truncate(term_width);
        println!("{welcome_msg}\r");
    }

    pub fn draw_row(&self, row: &Row) {
        let width = self.terminal.size().width as usize;
        let start = self.offset.x;
        let end = start.saturating_add(width);
        let row = row.render(start, end);
        println!("{row}\r");
    }

    /// Where the handling logics go.
    fn process_keypress(&mut self) -> Result<(), Error> {
        let pressed_key = Terminal::read_key()?;
        match pressed_key {
            // NOTE: Getting a `quit` signal isn't an error.
            Key::Ctrl('q') => {
                #[allow(clippy::arithmetic_side_effects)]
                if self.quit_times > 0 && self.document.is_dirty() {
                    self.status_message = StatusMessage::from(format!(
                        "WARN: File has unsaved changes! Press Ctrl-Q {} more times to quit.",
                        self.quit_times
                    ));
                    self.quit_times -= 1;
                    return Ok(());
                }
                self.should_quit = true;
            }
            Key::Ctrl('s') => self.save(),
            Key::Char(c) => {
                self.document.insert(&self.cursor_position, c);
                // So that we don't insert backward.
                self.move_cursor(Key::Right);
            }
            Key::Delete => self.document.delete(&self.cursor_position),
            // Backspace is a combination of going left and deleting.
            Key::Backspace => {
                if self.cursor_position.x > 0 || self.cursor_position.y > 0 {
                    self.move_cursor(Key::Left);
                    self.document.delete(&self.cursor_position);
                }
            }
            Key::Up
            | Key::Down
            | Key::Left
            | Key::Right
            | Key::PageUp
            | Key::PageDown
            | Key::End
            | Key::Home => self.move_cursor(pressed_key),
            _ => (),
        }
        self.scroll();
        // The user aborted the quit sequence.
        if self.quit_times < QUIT_TIMES {
            self.quit_times = QUIT_TIMES;
            self.status_message.clear();
        }
        Ok(())
    }

    fn scroll(&mut self) {
        let Position { x, y } = self.cursor_position;
        let width = self.terminal.size().width as usize;
        let height = self.terminal.size().height as usize;

        // Check if the cursor has moved outside of the visible window,
        // and if so, adjust offset so that the cursor is just inside the visible window.
        if y < self.offset.y {
            self.offset.y = y;
        } else if y >= self.offset.y.saturating_add(height) {
            self.offset.y = y.saturating_sub(height).saturating_add(1);
        }
        if x < self.offset.x {
            self.offset.x = x;
        } else if x >= self.offset.x.saturating_add(width) {
            self.offset.x = x.saturating_sub(width).saturating_add(1);
        }
    }

    fn move_cursor(&mut self, key: Key) {
        let Position { mut x, mut y } = self.cursor_position;
        let term_height = self.terminal.size().height as usize;
        // The cursor is allowed to move to the last row of the document.
        let doc_height = self.document.len();
        let mut row_width = if let Some(row) = self.document.row(y) {
            row.len()
        } else {
            0
        };
        match key {
            Key::Up => y = y.saturating_sub(1),
            Key::Down => {
                // Prevent the cursor from keep going down after the last row.
                if y < doc_height {
                    y = y.saturating_add(1);
                }
            }
            #[allow(clippy::arithmetic_side_effects)]
            Key::Left => {
                if x > 0 {
                    x -= 1;
                } else if y > 0 {
                    // Left at the beginning of the line moves to the end of the previous line.
                    y -= 1;
                    if let Some(row) = self.document.row(y) {
                        x = row.len();
                    } else {
                        x = 0;
                    }
                }
            }
            #[allow(clippy::arithmetic_side_effects)]
            Key::Right => {
                // Right at the end of the line moves to the beginning of the next line.
                if x < row_width {
                    x += 1;
                } else if y < doc_height {
                    y += 1;
                    x = 0;
                }
            }
            Key::PageUp => {
                y = {
                    if y > term_height {
                        y.saturating_sub(term_height)
                    } else {
                        0
                    }
                }
            }
            Key::PageDown => {
                y = {
                    if y.saturating_add(term_height) < doc_height {
                        y.saturating_add(term_height)
                    } else {
                        doc_height
                    }
                }
            }
            Key::Home => x = 0,
            Key::End => x = row_width,
            _ => (),
        }
        // Users may move the cursor from a long line to a short line.
        // We have to prevent the cursor from going beyond the end of the line.
        row_width = if let Some(row) = self.document.row(y) {
            row.len()
        } else {
            0
        };
        if x > row_width {
            x = row_width;
        }

        self.cursor_position = Position { x, y };
    }

    fn draw_status_bar(&self) {
        let modified_indicator = if self.document.is_dirty() {
            " (modified)"
        } else {
            ""
        };
        let filename = if let Some(name) = &self.document.filename {
            let mut name = name.clone();
            name.truncate(20);
            name
        } else {
            "[No Name]".to_owned()
        };
        let mut status = format!(
            "{filename} - {} lines{modified_indicator}",
            self.document.len()
        );
        let line_indicator = format!(
            "{}/{}",
            self.cursor_position.y.saturating_add(1), /* 1-based */
            self.document.len()
        );
        #[allow(clippy::arithmetic_side_effects)]
        let len = status.len() + line_indicator.len();
        let term_width = self.terminal.size().width as usize;
        status.push_str(&" ".repeat(term_width.saturating_sub(len)));
        // XXX: Isn't status always less than or equal to term_width?
        status.truncate(term_width);
        // The current line number is aligned to the right edge.
        status = format!("{status}{line_indicator}");
        Terminal::set_bg_color(STATUS_BG_COLOR);
        Terminal::set_fg_color(STATUS_FG_COLOR);
        println!("{status}\r");
        Terminal::reset_bg_color();
        Terminal::reset_fg_color();
    }

    fn draw_message_bar(&self) {
        Terminal::clear_current_line();
        let message = &self.status_message;
        if message.time.elapsed() < Duration::from_secs(5) {
            let mut text = message.text.clone();
            text.truncate(self.terminal.size().width as usize);
            print!("{text}");
        }
    }

    /// Prompt the user for input. `None` is returned if the user cancels the prompt.
    /// # Errors
    /// Returns an error if the user input can't be read.
    fn prompt(&mut self, prompt: &str) -> Result<Option<String>, Error> {
        let mut result = String::new();
        loop {
            self.status_message = StatusMessage::from(format!("{prompt}{result}"));
            self.refresh_screen()?;
            match Terminal::read_key()? {
                Key::Backspace => {
                    if !result.is_empty() {
                        result.pop();
                    }
                }
                // Enter is pressed; prompt is done.
                Key::Char('\n') => break,
                Key::Char(c) => {
                    if !c.is_control() {
                        result.push(c);
                    }
                }
                Key::Esc => {
                    result.clear();
                    break;
                }
                _ => (),
            }
        }
        self.status_message.clear();
        if result.is_empty() {
            Ok(None)
        } else {
            Ok(Some(result))
        }
    }

    fn save(&mut self) {
        // If the file has no name, prompt the user for one.
        if self.document.filename.is_none() {
            let new_name = self.prompt("Save as: ").unwrap_or(None);
            if new_name.is_none() {
                self.status_message = StatusMessage::from("Save aborted.".to_owned());
                return;
            }
            self.document.filename = new_name;
        }
        let msg = if self.document.save().is_ok() {
            "File saved sucessfully."
        } else {
            "Error writing file!"
        };
        self.status_message = StatusMessage::from(msg.to_owned());
    }
}

fn die(e: &Error) {
    Terminal::clear_screen();
    panic!("{}", e);
}
