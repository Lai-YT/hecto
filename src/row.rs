use std::cmp;

use unicode_segmentation::UnicodeSegmentation;

#[derive(Default)]
pub struct Row {
    string: String,
    len: usize,
}

impl From<&str> for Row {
    fn from(s: &str) -> Self {
        let mut row = Self {
            string: String::from(s),
            len: 0,
        };
        row.update_len();
        row
    }
}

impl Row {
    #[must_use]
    pub fn render(&self, start: usize, end: usize) -> String {
        // Get the actual end of such row.
        let end = cmp::min(end, self.string.len());
        // In case that `start` is greater than `end`, we want to return an empty string.
        let start = cmp::min(start, end);
        let mut result = String::new();
        for grapheme in self.string[..]
            .graphemes(true)
            .skip(start /* the ones to the left of the screen */)
            .take(end - start /* the visible portion of the row */)
        {
            // A tab is converted to a single space.
            // NOTE: If converting to multiple spaces, special care would be needed to
            // maintain the cursor position, as well as leaving it as it is.
            result.push_str(if grapheme == "\t" { " " } else { grapheme });
        }
        result
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.len
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// To avoid recomputing the length of the row every time we need it.
    fn update_len(&mut self) {
        self.len = self.string[..].graphemes(true).count();
    }

    pub fn insert(&mut self, at: usize, c: char) {
        if at >= self.len() {
            self.string.push(c);
        } else {
            // Splits the string into two half, inserts the character after the
            // first part, and then append the second part.
            let mut result: String = self.string[..].graphemes(true).take(at).collect();
            let reminder: String = self.string[..].graphemes(true).skip(at).collect();
            result.push(c);
            result.push_str(&reminder);
            self.string = result;
        }
        self.update_len();
    }

    pub fn delete(&mut self, at: usize) {
        if at >= self.len() {
            return;
        }
        let mut result: String = self.string[..].graphemes(true).take(at).collect();
        let remainder: String = self.string[..].graphemes(true).skip(at + 1).collect();
        result.push_str(&remainder);
        self.string = result;
        self.update_len();
    }

    pub fn append(&mut self, new: &Self) {
        self.string = format!("{}{}", self.string, new.string);
        self.update_len();
    }

    /// Truncates the current row up until a given index, and returns another row with
    /// everything behind that index.
    #[must_use]
    pub fn split(&mut self, at: usize) -> Self {
        let beginning: String = self.string[..].graphemes(true).take(at).collect();
        let remainder: String = self.string[..].graphemes(true).skip(at).collect();
        self.string = beginning;
        self.update_len();
        Self::from(&remainder[..])
    }

    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        self.string.as_bytes()
    }
}