//! Character-budgeted rendering shared by the runtime formatter
//! (`value::value_to_string`) and the debugger's slot summaries.

/// Character-budgeted string builder. The budget is enforced while
/// appending — a huge string or array never materializes in full before
/// truncation (design §5.3).
pub struct BoundedText {
    out: String,
    remaining: usize,
    pub truncated: bool,
}

impl BoundedText {
    pub fn new(limit: usize) -> Self {
        BoundedText {
            out: String::new(),
            remaining: limit,
            truncated: false,
        }
    }

    pub fn push(&mut self, text: &str) {
        if self.truncated {
            return;
        }
        for ch in text.chars() {
            if self.remaining == 0 {
                self.truncated = true;
                return;
            }
            self.out.push(ch);
            self.remaining -= 1;
        }
    }

    pub fn finish(mut self) -> String {
        if self.truncated {
            // The ellipsis is part of the advertised character budget.
            self.out.pop();
            self.out.push('…');
        }
        self.out
    }
}
