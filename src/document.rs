use crate::Position;
use crate::Row;
use std::fs;
use std::io::{Error, Write};

#[derive(Default)]
pub struct Document {
    rows: Vec<Row>,
    pub filename: Option<String>,
    /// Whether the document has been modified since the last save.
    is_dirty: bool,
}

impl Document {
    /// # Errors
    /// Returns an error if the file can't be read.
    pub fn open(filename: &str) -> Result<Self, std::io::Error> {
        let content = std::fs::read_to_string(filename)?;
        let mut rows = Vec::new();
        for value in content.lines() {
            rows.push(Row::from(value));
        }
        Ok(Self {
            rows,
            filename: Some(filename.to_string()),
            is_dirty: false,
        })
    }

    #[must_use]
    pub fn row(&self, index: usize) -> Option<&Row> {
        self.rows.get(index)
    }

    /// Whether the document is empty or no documents have been loaded.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// # Panics
    /// Panics if trying to insert pass the end of the row.
    pub fn insert(&mut self, at: &Position, c: char) {
        if at.y > self.len() {
            return;
        }
        self.is_dirty = true;
        if c == '\n' {
            self.insert_newline(at);
            return;
        }
        // If adding to the end of the file, push a new row with such
        // character as the first character; otherwise, take that row
        // and insert to the corresponding position.
        if at.y == self.len() {
            let mut row = Row::default();
            row.insert(0, c);
            self.rows.push(row);
        } else {
            let row = self.rows.get_mut(at.y).unwrap();
            row.insert(at.x, c);
        }
    }

    /// # Notes
    /// The dirty flag is not touched.
    fn insert_newline(&mut self, at: &Position) {
        // NOTE: Navigating to one row below the last is allowed.
        if at.y == self.len() {
            self.rows.push(Row::default());
            return;
        }
        // This works even at the end of a line, with `new_row` being empty.
        let new_row = self.rows.get_mut(at.y).unwrap().split(at.x);
        self.rows.insert(at.y + 1, new_row);
    }

    /// # Panics
    /// Panics if trying to delete pass the end of the row.
    pub fn delete(&mut self, at: &Position) {
        if at.y >= self.len() {
            return;
        }
        self.is_dirty = true;
        // If deleting at the end of the row, the next row is moved up.
        if at.x == self.rows.get(at.y).unwrap().len() && self.is_not_last_row(at) {
            let next_row = self.rows.remove(at.y + 1);
            let this_row = self.rows.get_mut(at.y).unwrap();
            this_row.append(&next_row);
        } else {
            let this_row = self.rows.get_mut(at.y).unwrap();
            this_row.delete(at.x);
        }
    }

    fn is_not_last_row(&self, at: &Position) -> bool {
        at.y < self.len() - 1
    }

    /// # Errors
    /// Returns an error if the file doesn't exist and can't be created, or can't
    /// be written.
    pub fn save(&mut self) -> Result<(), Error> {
        if let Some(filename) = &self.filename {
            let mut file = fs::File::create(filename)?;
            for row in &self.rows {
                file.write_all(row.as_bytes())?;
                file.write_all(b"\n")?;
            }
            self.is_dirty = false;
        }
        Ok(())
    }

    #[must_use]
    pub fn is_dirty(&self) -> bool {
        self.is_dirty
    }
}