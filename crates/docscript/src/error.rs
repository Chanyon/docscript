use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub message: String,
    pub line: usize,
    pub column: usize,
}

impl Diagnostic {
    pub fn new(message: impl Into<String>, line: usize, column: usize) -> Self {
        Self {
            message: message.into(),
            line,
            column,
        }
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "第 {} 行第 {} 列：{}",
            self.line.max(1),
            self.column.max(1),
            self.message
        )
    }
}

impl std::error::Error for Diagnostic {}

pub type Result<T> = std::result::Result<T, Diagnostic>;
