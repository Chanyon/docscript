use crate::error::{Diagnostic, Result};

const NESTED_BLOCK_FNS: &[&str] = &["list"];

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    Ident(String),
    BlockIdent(String),
    Int(i64),
    Float(f64),
    Dimension(f64, String),
    Color(String),
    Str(String),
    Text(String),
    At,
    Reference(String),
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Comma,
    Colon,
    Semicolon,
    Dot,
    DotDot,
    DotDotEq,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    EqEq,
    NotEq,
    Lt,
    Gt,
    LtEq,
    GtEq,
    AndAnd,
    OrOr,
    Bang,
    Eq,
    Eof,
}

#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub line: usize,
    pub column: usize,
    pub start: usize,
    pub end: usize,
}

impl Token {
    pub fn describe(&self) -> String {
        match &self.kind {
            TokenKind::Ident(name) => format!("标识符 `{name}`"),
            TokenKind::BlockIdent(name) => format!("嵌套块 `{name}`"),
            TokenKind::Int(_) | TokenKind::Float(_) => "数字".to_string(),
            TokenKind::Dimension(_, unit) => format!("带单位的数字 `{unit}`"),
            TokenKind::Color(color) => format!("颜色 `{color}`"),
            TokenKind::Str(_) | TokenKind::Text(_) => "文本".to_string(),
            TokenKind::At => "`@`".to_string(),
            TokenKind::Reference(name) => format!("引用 `{name}`"),
            TokenKind::LParen => "`(`".to_string(),
            TokenKind::RParen => "`)`".to_string(),
            TokenKind::LBrace => "`{`".to_string(),
            TokenKind::RBrace => "`}`".to_string(),
            TokenKind::LBracket => "`[`".to_string(),
            TokenKind::RBracket => "`]`".to_string(),
            TokenKind::Comma => "`,`".to_string(),
            TokenKind::Colon => "`:`".to_string(),
            TokenKind::Semicolon => "`;`".to_string(),
            TokenKind::Dot => "`.`".to_string(),
            TokenKind::DotDot => "`..`".to_string(),
            TokenKind::DotDotEq => "`..=`".to_string(),
            TokenKind::Plus => "`+`".to_string(),
            TokenKind::Minus => "`-`".to_string(),
            TokenKind::Star => "`*`".to_string(),
            TokenKind::Slash => "`/`".to_string(),
            TokenKind::Percent => "`%`".to_string(),
            TokenKind::EqEq => "`==`".to_string(),
            TokenKind::NotEq => "`!=`".to_string(),
            TokenKind::Lt => "`<`".to_string(),
            TokenKind::Gt => "`>`".to_string(),
            TokenKind::LtEq => "`<=`".to_string(),
            TokenKind::GtEq => "`>=`".to_string(),
            TokenKind::AndAnd => "`&&`".to_string(),
            TokenKind::OrOr => "`||`".to_string(),
            TokenKind::Bang => "`!`".to_string(),
            TokenKind::Eq => "`=`".to_string(),
            TokenKind::Eof => "文件结尾".to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum Mode {
    Code,
    Text { interp_depth: usize },
    Inline { paren_depth: usize },
    NestedBlock { brace_depth: usize },
}

pub struct Lexer<'a> {
    src: &'a str,
    offset: usize,
    line: usize,
    column: usize,
    modes: Vec<Mode>,
}

impl<'a> Lexer<'a> {
    pub fn new(src: &'a str) -> Self {
        Self {
            src,
            offset: 0,
            line: 1,
            column: 1,
            modes: vec![Mode::Code],
        }
    }

    pub fn enter_text(&mut self) {
        if let Some(Mode::NestedBlock { brace_depth }) = self.modes.last_mut() {
            *brace_depth = brace_depth.saturating_sub(1);
        }
        self.modes.push(Mode::Text { interp_depth: 0 });
    }

    pub fn next_token(&mut self) -> Result<Token> {
        let mode = *self.modes.last().unwrap_or(&Mode::Code);
        match mode {
            Mode::Text { interp_depth: 0 } => self.next_text_token(),
            Mode::Text { interp_depth } => {
                let token = self.scan_code_token()?;
                let new_depth = match token.kind {
                    TokenKind::LBrace => Some(interp_depth + 1),
                    TokenKind::RBrace => Some(interp_depth.saturating_sub(1)),
                    _ => None,
                };
                if let Some(depth) = new_depth
                    && let Some(Mode::Text { interp_depth }) = self.modes.last_mut()
                {
                    *interp_depth = depth;
                }
                Ok(token)
            }
            Mode::Inline { paren_depth } => {
                let token = self.scan_code_token()?;
                match token.kind {
                    TokenKind::LParen => {
                        if let Some(Mode::Inline { paren_depth }) = self.modes.last_mut() {
                            *paren_depth += 1;
                        }
                    }
                    TokenKind::RParen => {
                        if paren_depth <= 1 {
                            self.modes.pop();
                        } else if let Some(Mode::Inline { paren_depth }) = self.modes.last_mut() {
                            *paren_depth -= 1;
                        }
                    }
                    _ => {}
                }
                Ok(token)
            }
            Mode::NestedBlock { brace_depth } => {
                let token = self.scan_code_token()?;
                match token.kind {
                    TokenKind::LBrace => {
                        if let Some(Mode::NestedBlock { brace_depth }) = self.modes.last_mut() {
                            *brace_depth += 1;
                        }
                    }
                    TokenKind::RBrace => {
                        if brace_depth <= 1 {
                            self.modes.pop();
                        } else if let Some(Mode::NestedBlock { brace_depth }) =
                            self.modes.last_mut()
                        {
                            *brace_depth -= 1;
                        }
                    }
                    _ => {}
                }
                Ok(token)
            }
            Mode::Code => self.scan_code_token(),
        }
    }

    fn next_text_token(&mut self) -> Result<Token> {
        let line = self.line;
        let column = self.column;
        let start = self.offset;

        if let Some(name) = self.peek_nested_block_name() {
            while let Some(ch) = self.peek() {
                if is_ident_continue(ch) {
                    self.bump();
                } else {
                    break;
                }
            }
            self.modes.push(Mode::NestedBlock { brace_depth: 0 });
            return Ok(Token {
                kind: TokenKind::BlockIdent(name),
                line,
                column,
                start,
                end: self.offset,
            });
        }

        match self.peek() {
            None => Err(Diagnostic::new("文本块没有闭合", line, column)),
            Some('}') => {
                self.bump();
                self.modes.pop();
                Ok(Token {
                    kind: TokenKind::RBrace,
                    line,
                    column,
                    start,
                    end: self.offset,
                })
            }
            Some('{') => {
                self.bump();
                if let Some(Mode::Text { interp_depth }) = self.modes.last_mut() {
                    *interp_depth = 1;
                }
                Ok(Token {
                    kind: TokenKind::LBrace,
                    line,
                    column,
                    start,
                    end: self.offset,
                })
            }
            Some(ch) if is_ident_start(ch) && self.ident_followed_by_paren() => {
                let mut name = String::new();
                while let Some(ch) = self.peek() {
                    if is_ident_continue(ch) {
                        name.push(ch);
                        self.bump();
                    } else {
                        break;
                    }
                }
                self.modes.push(Mode::Inline { paren_depth: 0 });
                Ok(Token {
                    kind: TokenKind::Ident(name),
                    line,
                    column,
                    start,
                    end: self.offset,
                })
            }
            Some('@') if self.peek_reference_name().is_some() => {
                self.bump();
                let mut name = String::new();
                while let Some(ch) = self.peek() {
                    if is_ident_continue(ch) {
                        name.push(ch);
                        self.bump();
                    } else {
                        break;
                    }
                }
                Ok(Token {
                    kind: TokenKind::Reference(name),
                    line,
                    column,
                    start,
                    end: self.offset,
                })
            }
            Some(_) => {
                let mut text = String::new();
                loop {
                    if self.peek_nested_block_name().is_some() {
                        break;
                    }
                    match self.peek() {
                        None | Some('}') | Some('{') => break,
                        Some('\\') => {
                            self.bump();
                            match self.peek() {
                                Some(next)
                                    if matches!(next, '{' | '}' | '\\' | '(' | ')' | '@') =>
                                {
                                    self.bump();
                                    text.push(next);
                                }
                                _ => text.push('\\'),
                            }
                        }
                        Some('@') if self.peek_reference_name().is_some() => break,
                        Some(ch) => {
                            if is_ident_start(ch) && self.ident_followed_by_paren() {
                                break;
                            }
                            self.bump();
                            text.push(ch);
                        }
                    }
                }
                if text.is_empty() {
                    return Err(Diagnostic::new("无法读取文本", line, column));
                }
                Ok(Token {
                    kind: TokenKind::Text(text),
                    line,
                    column,
                    start,
                    end: self.offset,
                })
            }
        }
    }

    fn scan_code_token(&mut self) -> Result<Token> {
        self.skip_trivia();

        let line = self.line;
        let column = self.column;
        let start = self.offset;

        let Some(ch) = self.peek() else {
            return Ok(Token {
                kind: TokenKind::Eof,
                line,
                column,
                start,
                end: start,
            });
        };

        let kind = match ch {
            '"' => TokenKind::Str(self.scan_string()?),
            '`' => TokenKind::Str(self.scan_raw_block()?),
            ch if ch.is_ascii_digit() => self.scan_number()?,
            ch if is_ident_start(ch) => {
                let mut name = String::new();
                while let Some(ch) = self.peek() {
                    if is_ident_continue(ch) {
                        name.push(ch);
                        self.bump();
                    } else {
                        break;
                    }
                }
                TokenKind::Ident(name)
            }
            '@' => self.single(TokenKind::At),
            '#' => {
                self.bump();
                let mut color = String::from("#");
                while let Some(ch) = self.peek() {
                    if ch.is_ascii_hexdigit() {
                        color.push(ch);
                        self.bump();
                    } else {
                        break;
                    }
                }
                if color.len() == 1 {
                    return Err(Diagnostic::new(
                        "`#` 后需要颜色十六进制值，例如 #333 或 #1f2937",
                        line,
                        column,
                    ));
                }
                TokenKind::Color(color)
            }
            '(' => self.single(TokenKind::LParen),
            ')' => self.single(TokenKind::RParen),
            '{' => self.single(TokenKind::LBrace),
            '}' => self.single(TokenKind::RBrace),
            '[' => self.single(TokenKind::LBracket),
            ']' => self.single(TokenKind::RBracket),
            ',' => self.single(TokenKind::Comma),
            ':' => self.single(TokenKind::Colon),
            ';' => self.single(TokenKind::Semicolon),
            '+' => self.single(TokenKind::Plus),
            '-' => self.single(TokenKind::Minus),
            '*' => self.single(TokenKind::Star),
            '%' => self.single(TokenKind::Percent),
            '/' => self.single(TokenKind::Slash),
            '.' => {
                self.bump();
                if self.peek() == Some('.') {
                    self.bump();
                    if self.peek() == Some('=') {
                        self.bump();
                        TokenKind::DotDotEq
                    } else {
                        TokenKind::DotDot
                    }
                } else {
                    TokenKind::Dot
                }
            }
            '=' => {
                self.bump();
                if self.peek() == Some('=') {
                    self.bump();
                    TokenKind::EqEq
                } else {
                    TokenKind::Eq
                }
            }
            '!' => {
                self.bump();
                if self.peek() == Some('=') {
                    self.bump();
                    TokenKind::NotEq
                } else {
                    TokenKind::Bang
                }
            }
            '<' => {
                self.bump();
                if self.peek() == Some('=') {
                    self.bump();
                    TokenKind::LtEq
                } else {
                    TokenKind::Lt
                }
            }
            '>' => {
                self.bump();
                if self.peek() == Some('=') {
                    self.bump();
                    TokenKind::GtEq
                } else {
                    TokenKind::Gt
                }
            }
            '&' => {
                self.bump();
                if self.peek() == Some('&') {
                    self.bump();
                    TokenKind::AndAnd
                } else {
                    return Err(Diagnostic::new("单个 `&` 无效，请使用 `&&`", line, column));
                }
            }
            '|' => {
                self.bump();
                if self.peek() == Some('|') {
                    self.bump();
                    TokenKind::OrOr
                } else {
                    return Err(Diagnostic::new("单个 `|` 无效，请使用 `||`", line, column));
                }
            }
            other => {
                return Err(Diagnostic::new(
                    format!("无法识别的字符 `{other}`"),
                    line,
                    column,
                ));
            }
        };

        Ok(Token {
            kind,
            line,
            column,
            start,
            end: self.offset,
        })
    }

    fn single(&mut self, kind: TokenKind) -> TokenKind {
        self.bump();
        kind
    }

    fn skip_trivia(&mut self) {
        loop {
            while let Some(ch) = self.peek() {
                if ch.is_whitespace() {
                    self.bump();
                } else {
                    break;
                }
            }

            if self.starts_with("//") {
                self.bump();
                self.bump();
                while let Some(ch) = self.peek() {
                    self.bump();
                    if ch == '\n' {
                        break;
                    }
                }
                continue;
            }

            if self.starts_with("/*") {
                self.bump();
                self.bump();
                while self.peek().is_some() {
                    if self.starts_with("*/") {
                        self.bump();
                        self.bump();
                        break;
                    }
                    self.bump();
                }
                continue;
            }

            break;
        }
    }

    fn scan_string(&mut self) -> Result<String> {
        let line = self.line;
        let column = self.column;
        self.bump();
        let mut out = String::new();
        loop {
            match self.peek() {
                None => return Err(Diagnostic::new("字符串没有闭合", line, column)),
                Some('"') => {
                    self.bump();
                    return Ok(out);
                }
                Some('\\') => {
                    self.bump();
                    match self.peek() {
                        Some('n') => {
                            self.bump();
                            out.push('\n');
                        }
                        Some('r') => {
                            self.bump();
                            out.push('\r');
                        }
                        Some('t') => {
                            self.bump();
                            out.push('\t');
                        }
                        Some('"') => {
                            self.bump();
                            out.push('"');
                        }
                        Some('\\') => {
                            self.bump();
                            out.push('\\');
                        }
                        Some('u') => {
                            self.bump();
                            if self.peek() != Some('{') {
                                return Err(Diagnostic::new(
                                    "Unicode 转义格式应为 \\u{1F600}",
                                    self.line,
                                    self.column,
                                ));
                            }
                            self.bump();
                            let mut hex = String::new();
                            while let Some(ch) = self.peek() {
                                if ch == '}' {
                                    break;
                                }
                                hex.push(ch);
                                self.bump();
                            }
                            if self.peek() != Some('}') {
                                return Err(Diagnostic::new(
                                    "Unicode 转义没有闭合",
                                    self.line,
                                    self.column,
                                ));
                            }
                            self.bump();
                            let code = u32::from_str_radix(&hex, 16).map_err(|_| {
                                Diagnostic::new("无效的 Unicode 转义", line, column)
                            })?;
                            let ch = char::from_u32(code).ok_or_else(|| {
                                Diagnostic::new("无效的 Unicode 码点", line, column)
                            })?;
                            out.push(ch);
                        }
                        Some(ch) => {
                            self.bump();
                            out.push(ch);
                        }
                        None => return Err(Diagnostic::new("字符串没有闭合", line, column)),
                    }
                }
                Some(ch) => {
                    self.bump();
                    out.push(ch);
                }
            }
        }
    }

    fn scan_raw_block(&mut self) -> Result<String> {
        let line = self.line;
        let column = self.column;
        self.bump();
        let mut out = String::new();
        loop {
            match self.peek() {
                None => {
                    return Err(Diagnostic::new(
                        "原始文本块没有闭合（缺少 `）",
                        line,
                        column,
                    ));
                }
                Some('`') => {
                    self.bump();
                    return Ok(normalize_raw_block(out));
                }
                Some(ch) => {
                    self.bump();
                    out.push(ch);
                }
            }
        }
    }

    fn scan_number(&mut self) -> Result<TokenKind> {
        let line = self.line;
        let column = self.column;
        let mut text = String::new();
        while let Some(ch) = self.peek() {
            if ch.is_ascii_digit() || ch == '_' {
                if ch != '_' {
                    text.push(ch);
                }
                self.bump();
            } else {
                break;
            }
        }

        let mut is_float = false;
        if self.peek() == Some('.') && self.peek_nth(1).is_some_and(|ch| ch.is_ascii_digit()) {
            is_float = true;
            text.push('.');
            self.bump();
            while let Some(ch) = self.peek() {
                if ch.is_ascii_digit() || ch == '_' {
                    if ch != '_' {
                        text.push(ch);
                    }
                    self.bump();
                } else {
                    break;
                }
            }
        }

        let mut unit = String::new();
        while let Some(ch) = self.peek() {
            if ch.is_ascii_alphabetic() || ch == '%' {
                unit.push(ch.to_ascii_lowercase());
                self.bump();
            } else {
                break;
            }
        }

        if !unit.is_empty() {
            let value = text
                .parse::<f64>()
                .map_err(|_| Diagnostic::new("无效的数字", line, column))?;
            return Ok(TokenKind::Dimension(value, unit));
        }

        if is_float {
            text.parse::<f64>()
                .map(TokenKind::Float)
                .map_err(|_| Diagnostic::new("无效的数字", line, column))
        } else {
            text.parse::<i64>()
                .map(TokenKind::Int)
                .map_err(|_| Diagnostic::new("无效的数字", line, column))
        }
    }

    fn peek_nested_block_name(&self) -> Option<String> {
        if !self.at_text_line_start() {
            return None;
        }
        let rest = &self.src[self.offset..];
        let mut chars = rest.chars();
        let first = chars.next()?;
        if !is_ident_start(first) {
            return None;
        }
        let mut name = String::from(first);
        for ch in chars {
            if is_ident_continue(ch) {
                name.push(ch);
            } else {
                break;
            }
        }
        if !NESTED_BLOCK_FNS.contains(&name.as_str()) {
            return None;
        }
        let after = &rest[name.len()..];
        for ch in after.chars() {
            if ch == ' ' || ch == '\t' {
                continue;
            }
            return (ch == '{' || ch == '(').then_some(name);
        }
        None
    }

    fn at_text_line_start(&self) -> bool {
        let before = &self.src[..self.offset];
        let line_start = before.rfind('\n').map(|index| index + 1).unwrap_or(0);
        before[line_start..]
            .chars()
            .all(|ch| ch == ' ' || ch == '\t')
    }
    fn ident_followed_by_paren(&self) -> bool {
        let mut chars = self.src[self.offset..].chars();
        match chars.next() {
            Some(ch) if is_ident_start(ch) => {}
            _ => return false,
        }
        for ch in chars {
            if is_ident_continue(ch) {
                continue;
            }
            return ch == '(';
        }
        false
    }

    fn peek_reference_name(&self) -> Option<String> {
        let mut chars = self.src[self.offset..].chars();
        if chars.next()? != '@' {
            return None;
        }
        let first = chars.next()?;
        if !is_ident_start(first) {
            return None;
        }
        let mut name = String::new();
        name.push(first);
        for ch in chars {
            if is_ident_continue(ch) {
                name.push(ch);
            } else {
                break;
            }
        }
        if name.is_empty() || name == "set" || name == "define" {
            None
        } else {
            Some(name)
        }
    }

    fn peek(&self) -> Option<char> {
        self.src[self.offset..].chars().next()
    }

    fn peek_nth(&self, index: usize) -> Option<char> {
        self.src[self.offset..].chars().nth(index)
    }

    fn starts_with(&self, text: &str) -> bool {
        self.src[self.offset..].starts_with(text)
    }

    fn bump(&mut self) -> Option<char> {
        let ch = self.peek()?;
        self.offset += ch.len_utf8();
        if ch == '\n' {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }
        Some(ch)
    }
}

fn normalize_raw_block(text: String) -> String {
    let text = text.replace("\r\n", "\n").replace('\r', "\n");
    let mut lines: Vec<String> = text.split('\n').map(ToString::to_string).collect();
    while lines.first().is_some_and(|line| line.trim().is_empty()) {
        lines.remove(0);
    }
    while lines.last().is_some_and(|line| line.trim().is_empty()) {
        lines.pop();
    }
    if lines.is_empty() {
        return String::new();
    }

    let indent = lines
        .iter()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            line.chars()
                .take_while(|ch| *ch == ' ' || *ch == '\t')
                .count()
        })
        .min()
        .unwrap_or(0);
    if indent > 0 {
        lines = lines
            .into_iter()
            .map(|line| {
                if line.len() >= indent {
                    line[indent..].to_string()
                } else {
                    line
                }
            })
            .collect();
    }

    let backslashes = lines
        .iter()
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.chars().take_while(|ch| *ch == '\\').count())
        .min()
        .unwrap_or(0);
    if backslashes > 0 {
        lines = lines
            .into_iter()
            .map(|line| {
                if line.starts_with(&"\\".repeat(backslashes)) {
                    line[backslashes..].to_string()
                } else {
                    line
                }
            })
            .collect();
    }

    lines.join("\n")
}

fn is_ident_start(ch: char) -> bool {
    ch.is_ascii_alphabetic() || ch == '_'
}

fn is_ident_continue(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}
