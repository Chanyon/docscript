use crate::ast::*;
use crate::error::{Diagnostic, Result};
use crate::lexer::{Lexer, Token, TokenKind};

pub const TEXT_BLOCK_FNS: &[&str] = &[
    "p",
    "paragraph",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "heading",
    "title",
    "subtitle",
    "quote",
    "item",
    "text",
    "span",
    "bold",
    "strong",
    "italic",
    "em",
    "code",
    "link",
    "caption",
    "label",
];

pub struct Parser<'a> {
    lexer: Lexer<'a>,
    current: Token,
    peeked: Option<Token>,
    allow_block_body: bool,
}

impl<'a> Parser<'a> {
    pub fn new(source: &'a str) -> Result<Self> {
        let mut lexer = Lexer::new(source);
        let current = lexer.next_token()?;
        Ok(Self {
            lexer,
            current,
            peeked: None,
            allow_block_body: true,
        })
    }

    pub fn parse_program(mut self) -> Result<Program> {
        let mut stmts = Vec::new();
        while !matches!(self.current.kind, TokenKind::Eof) {
            stmts.push(self.parse_stmt()?);
        }
        Ok(Program { stmts })
    }

    fn parse_stmt(&mut self) -> Result<Stmt> {
        match &self.current.kind {
            TokenKind::Ident(name) if name == "let" => {
                self.bump()?;
                let name = self.expect_ident("变量名")?;
                self.expect(TokenKind::Eq, "`=`")?;
                let value = self.parse_expr()?;
                self.eat_semicolon()?;
                Ok(Stmt::Let { name, value })
            }
            TokenKind::Ident(name) if name == "fn" => {
                self.bump()?;
                let name = self.expect_ident("函数名")?;
                let params = self.parse_params()?;
                let body = self.parse_code_block()?;
                Ok(Stmt::Fn { name, params, body })
            }
            TokenKind::At => {
                self.bump()?;
                let keyword = self.expect_ident("指令名")?;
                match keyword.as_str() {
                    "set" => self.parse_set_stmt(),
                    "define" => self.parse_define_stmt(),
                    _ => self.parse_reference_stmt(keyword),
                }
            }
            TokenKind::Ident(name) if name == "if" => {
                let statement = self.parse_if_stmt()?;
                Ok(statement)
            }
            TokenKind::Ident(name) if name == "for" => {
                self.bump()?;
                let name = self.expect_ident("循环变量名")?;
                let value_name = if self.eat(TokenKind::Comma)? {
                    Some(self.expect_ident("循环值变量名")?)
                } else {
                    None
                };
                self.expect_keyword("in")?;
                let saved = self.allow_block_body;
                self.allow_block_body = false;
                let iterable = self.parse_expr();
                self.allow_block_body = saved;
                let iterable = iterable?;
                let body = self.parse_code_block()?;
                Ok(Stmt::For {
                    name,
                    value_name,
                    iterable,
                    body,
                })
            }
            _ => {
                let expr = self.parse_expr()?;
                self.eat_semicolon()?;
                Ok(Stmt::Expr(expr))
            }
        }
    }

    fn parse_params(&mut self) -> Result<Vec<Param>> {
        self.expect(TokenKind::LParen, "`(`")?;
        let mut params = Vec::new();
        while !matches!(self.current.kind, TokenKind::RParen) {
            let param_name = self.expect_ident("参数名")?;
            let default = if self.eat(TokenKind::Eq)? {
                Some(self.parse_expr()?)
            } else {
                None
            };
            params.push(Param {
                name: param_name,
                default,
            });
            if !self.eat(TokenKind::Comma)? {
                break;
            }
        }
        self.expect(TokenKind::RParen, "`)`")?;
        Ok(params)
    }

    fn parse_set_stmt(&mut self) -> Result<Stmt> {
        let target = self.expect_ident("要设置的元素名")?;
        let suffix = if self.eat(TokenKind::Dot)? {
            Some(self.expect_ident("属性名")?)
        } else {
            None
        };
        let args = if matches!(self.current.kind, TokenKind::LParen) {
            self.parse_arguments()?
        } else {
            Vec::new()
        };
        let body = if matches!(self.current.kind, TokenKind::LBrace) {
            Some(self.parse_code_block()?)
        } else {
            None
        };
        self.eat_semicolon()?;
        Ok(Stmt::Set {
            target,
            suffix,
            args,
            body,
        })
    }

    fn parse_define_stmt(&mut self) -> Result<Stmt> {
        let name = self.expect_ident("定义元素名")?;
        let params = self.parse_params()?;
        let body = self.parse_code_block()?;
        Ok(Stmt::Define { name, params, body })
    }

    fn parse_reference_stmt(&mut self, name: String) -> Result<Stmt> {
        if !matches!(self.current.kind, TokenKind::LParen) {
            return Err(Diagnostic::new(
                format!("参考 `@{name}` 需要括号内容，例如 @{name}(text {{ ... }})"),
                self.current.line,
                self.current.column,
            ));
        }
        let args = self.parse_arguments()?;
        let body = if matches!(self.current.kind, TokenKind::LBrace) {
            Some(self.parse_code_block()?)
        } else {
            None
        };
        self.eat_semicolon()?;
        Ok(Stmt::Reference { name, args, body })
    }

    fn parse_arguments(&mut self) -> Result<Vec<Argument>> {
        self.expect(TokenKind::LParen, "`(`")?;
        let mut args = Vec::new();
        while !matches!(self.current.kind, TokenKind::RParen) {
            let arg_name = if matches!(self.current.kind, TokenKind::Ident(_))
                && matches!(self.peek()?.kind, TokenKind::Colon)
            {
                let arg_name = self.expect_ident("参数名")?;
                self.expect(TokenKind::Colon, "`:`")?;
                Some(arg_name)
            } else {
                None
            };
            let value = self.parse_expr()?;
            args.push(Argument {
                name: arg_name,
                value,
            });
            if !self.eat(TokenKind::Comma)? {
                break;
            }
        }
        self.expect(TokenKind::RParen, "`)`")?;
        Ok(args)
    }

    fn parse_if_stmt(&mut self) -> Result<Stmt> {
        self.bump()?;
        let saved = self.allow_block_body;
        self.allow_block_body = false;
        let condition = self.parse_expr();
        self.allow_block_body = saved;
        let condition = condition?;
        let then_branch = self.parse_code_block()?;
        let else_branch = if matches!(&self.current.kind, TokenKind::Ident(name) if name == "else")
        {
            self.bump()?;
            if matches!(&self.current.kind, TokenKind::Ident(name) if name == "if") {
                Some(vec![self.parse_if_stmt()?])
            } else {
                Some(self.parse_code_block()?)
            }
        } else {
            None
        };
        Ok(Stmt::If {
            condition,
            then_branch,
            else_branch,
        })
    }

    fn parse_code_block(&mut self) -> Result<Vec<Stmt>> {
        self.expect(TokenKind::LBrace, "`{`")?;
        let mut stmts = Vec::new();
        while !matches!(self.current.kind, TokenKind::RBrace | TokenKind::Eof) {
            stmts.push(self.parse_stmt()?);
        }
        if matches!(self.current.kind, TokenKind::Eof) {
            return Err(Diagnostic::new(
                "代码块没有闭合",
                self.current.line,
                self.current.column,
            ));
        }
        self.bump()?;
        Ok(stmts)
    }

    fn parse_text_block(&mut self) -> Result<Vec<TextPart>> {
        let mut parts = Vec::new();
        loop {
            match &self.current.kind {
                TokenKind::Text(text) => {
                    parts.push(TextPart::Text(text.clone()));
                    self.bump()?;
                }
                TokenKind::LBrace => {
                    self.bump()?;
                    let expr = self.parse_expr()?;
                    self.expect(TokenKind::RBrace, "`}`")?;
                    parts.push(TextPart::Expr(expr));
                }
                TokenKind::Reference(name) => {
                    let name = name.clone();
                    self.bump()?;
                    parts.push(TextPart::Reference(name));
                }
                TokenKind::BlockIdent(name) => {
                    let name = name.clone();
                    self.bump()?;
                    let call = self.parse_call_after_name(name.clone(), true)?;
                    if matches!(&call, Expr::Call { body: None, .. }) {
                        return Err(Diagnostic::new(
                            format!("嵌套块 `{name}` 需要花括号块"),
                            self.current.line,
                            self.current.column,
                        ));
                    }
                    parts.push(TextPart::Expr(call));
                }
                TokenKind::Ident(name) => {
                    let name = name.clone();
                    self.bump()?;
                    let call = self.parse_call_after_name(name, false)?;
                    parts.push(TextPart::Expr(call));
                }
                TokenKind::RBrace => {
                    self.bump()?;
                    break;
                }
                TokenKind::Eof => {
                    return Err(Diagnostic::new(
                        "文本块没有闭合",
                        self.current.line,
                        self.current.column,
                    ));
                }
                _ => {
                    return Err(Diagnostic::new(
                        format!("文本块中出现了意外的 {}", self.current.describe()),
                        self.current.line,
                        self.current.column,
                    ));
                }
            }
        }
        Ok(parts)
    }

    fn parse_expr(&mut self) -> Result<Expr> {
        self.parse_or()
    }

    fn parse_or(&mut self) -> Result<Expr> {
        let mut expr = self.parse_and()?;
        while matches!(self.current.kind, TokenKind::OrOr) {
            self.bump()?;
            let right = self.parse_and()?;
            expr = Expr::Binary {
                op: BinaryOp::Or,
                left: Box::new(expr),
                right: Box::new(right),
            };
        }
        Ok(expr)
    }

    fn parse_and(&mut self) -> Result<Expr> {
        let mut expr = self.parse_equality()?;
        while matches!(self.current.kind, TokenKind::AndAnd) {
            self.bump()?;
            let right = self.parse_equality()?;
            expr = Expr::Binary {
                op: BinaryOp::And,
                left: Box::new(expr),
                right: Box::new(right),
            };
        }
        Ok(expr)
    }

    fn parse_equality(&mut self) -> Result<Expr> {
        let mut expr = self.parse_comparison()?;
        loop {
            let op = match self.current.kind {
                TokenKind::EqEq => BinaryOp::Equal,
                TokenKind::NotEq => BinaryOp::NotEqual,
                _ => break,
            };
            self.bump()?;
            let right = self.parse_comparison()?;
            expr = Expr::Binary {
                op,
                left: Box::new(expr),
                right: Box::new(right),
            };
        }
        Ok(expr)
    }

    fn parse_comparison(&mut self) -> Result<Expr> {
        let mut expr = self.parse_range()?;
        loop {
            let op = match self.current.kind {
                TokenKind::Lt => BinaryOp::Less,
                TokenKind::LtEq => BinaryOp::LessEqual,
                TokenKind::Gt => BinaryOp::Greater,
                TokenKind::GtEq => BinaryOp::GreaterEqual,
                _ => break,
            };
            self.bump()?;
            let right = self.parse_range()?;
            expr = Expr::Binary {
                op,
                left: Box::new(expr),
                right: Box::new(right),
            };
        }
        Ok(expr)
    }

    fn parse_range(&mut self) -> Result<Expr> {
        let start = self.parse_additive()?;
        let inclusive = match self.current.kind {
            TokenKind::DotDot => false,
            TokenKind::DotDotEq => true,
            _ => return Ok(start),
        };
        self.bump()?;
        let end = self.parse_additive()?;
        Ok(Expr::Range {
            start: Box::new(start),
            end: Box::new(end),
            inclusive,
        })
    }

    fn parse_additive(&mut self) -> Result<Expr> {
        let mut expr = self.parse_multiplicative()?;
        loop {
            let op = match self.current.kind {
                TokenKind::Plus => BinaryOp::Add,
                TokenKind::Minus => BinaryOp::Subtract,
                _ => break,
            };
            self.bump()?;
            let right = self.parse_multiplicative()?;
            expr = Expr::Binary {
                op,
                left: Box::new(expr),
                right: Box::new(right),
            };
        }
        Ok(expr)
    }

    fn parse_multiplicative(&mut self) -> Result<Expr> {
        let mut expr = self.parse_unary()?;
        loop {
            let op = match self.current.kind {
                TokenKind::Star => BinaryOp::Multiply,
                TokenKind::Slash => BinaryOp::Divide,
                TokenKind::Percent => BinaryOp::Remainder,
                _ => break,
            };
            self.bump()?;
            let right = self.parse_unary()?;
            expr = Expr::Binary {
                op,
                left: Box::new(expr),
                right: Box::new(right),
            };
        }
        Ok(expr)
    }

    fn parse_unary(&mut self) -> Result<Expr> {
        match self.current.kind {
            TokenKind::Bang => {
                self.bump()?;
                Ok(Expr::Unary {
                    op: UnaryOp::Not,
                    expr: Box::new(self.parse_unary()?),
                })
            }
            TokenKind::Minus => {
                self.bump()?;
                Ok(Expr::Unary {
                    op: UnaryOp::Negate,
                    expr: Box::new(self.parse_unary()?),
                })
            }
            _ => self.parse_postfix(),
        }
    }

    fn parse_postfix(&mut self) -> Result<Expr> {
        let mut expr = self.parse_primary()?;
        loop {
            let class_call = match &expr {
                Expr::Member { target, name } => match target.as_ref() {
                    Expr::Ident(base) => Some((base.clone(), name.clone())),
                    _ => None,
                },
                _ => None,
            };
            if let Some((base, class_name)) = class_call
                && (matches!(self.current.kind, TokenKind::LParen)
                    || (self.allow_block_body && matches!(self.current.kind, TokenKind::LBrace)))
            {
                expr = self.parse_call_after_name(format!("{base}.{class_name}"), true)?;
                continue;
            }
            match self.current.kind {
                TokenKind::Dot => {
                    self.bump()?;
                    let name = self.expect_ident("成员名")?;
                    expr = Expr::Member {
                        target: Box::new(expr),
                        name,
                    };
                }
                TokenKind::LBracket => {
                    self.bump()?;
                    let index = self.parse_expr()?;
                    self.expect(TokenKind::RBracket, "`]`")?;
                    expr = Expr::Index {
                        target: Box::new(expr),
                        index: Box::new(index),
                    };
                }
                _ => break,
            }
        }
        Ok(expr)
    }

    fn parse_primary(&mut self) -> Result<Expr> {
        match &self.current.kind {
            TokenKind::Int(number) => {
                let number = *number;
                self.bump()?;
                Ok(Expr::Int(number))
            }
            TokenKind::Float(number) => {
                let number = *number;
                self.bump()?;
                Ok(Expr::Float(number))
            }
            TokenKind::Dimension(number, unit) => {
                let number = *number;
                let unit = unit.clone();
                self.bump()?;
                Ok(Expr::Dimension(number, unit))
            }
            TokenKind::Color(color) => {
                let color = color.clone();
                self.bump()?;
                Ok(Expr::Str(color))
            }
            TokenKind::At => {
                self.bump()?;
                let name = self.expect_ident("表达式")?;
                if name != "style" {
                    return Err(Diagnostic::new(
                        format!("未知表达式 `@{name}`，可用 `@style`"),
                        self.current.line,
                        self.current.column,
                    ));
                }
                if self.eat(TokenKind::LParen)? {
                    let expr = self.parse_expr()?;
                    self.expect(TokenKind::RParen, "`)`")?;
                    Ok(expr)
                } else {
                    self.parse_expr()
                }
            }
            TokenKind::Str(text) => {
                let text = text.clone();
                self.bump()?;
                Ok(Expr::Str(text))
            }
            TokenKind::Ident(name) => {
                let name = name.clone();
                self.bump()?;
                match name.as_str() {
                    "true" => return Ok(Expr::Bool(true)),
                    "false" => return Ok(Expr::Bool(false)),
                    "null" => return Ok(Expr::Null),
                    _ => {}
                }
                if matches!(self.current.kind, TokenKind::LParen)
                    || (self.allow_block_body && matches!(self.current.kind, TokenKind::LBrace))
                {
                    self.parse_call_after_name(name, true)
                } else {
                    Ok(Expr::Ident(name))
                }
            }
            TokenKind::LParen => {
                self.bump()?;
                let expr = self.parse_expr()?;
                self.expect(TokenKind::RParen, "`)`")?;
                Ok(expr)
            }
            TokenKind::LBracket => {
                self.bump()?;
                let mut values = Vec::new();
                while !matches!(self.current.kind, TokenKind::RBracket) {
                    values.push(self.parse_expr()?);
                    if !self.eat(TokenKind::Comma)? {
                        break;
                    }
                }
                self.expect(TokenKind::RBracket, "`]`")?;
                Ok(Expr::List(values))
            }
            TokenKind::LBrace => {
                self.bump()?;
                let mut values = Vec::new();
                while !matches!(self.current.kind, TokenKind::RBrace) {
                    let key = match &self.current.kind {
                        TokenKind::Ident(name) => {
                            let name = name.clone();
                            self.bump()?;
                            name
                        }
                        TokenKind::Str(text) => {
                            let text = text.clone();
                            self.bump()?;
                            text
                        }
                        _ => {
                            return Err(Diagnostic::new(
                                "对象键必须是标识符或字符串",
                                self.current.line,
                                self.current.column,
                            ));
                        }
                    };
                    self.expect(TokenKind::Colon, "`:`")?;
                    let value = self.parse_expr()?;
                    values.push((key, value));
                    if !self.eat(TokenKind::Comma)? && !self.eat(TokenKind::Semicolon)? {
                        if !matches!(self.current.kind, TokenKind::RBrace) {
                            continue;
                        }
                        break;
                    }
                }
                self.expect(TokenKind::RBrace, "`}`")?;
                Ok(Expr::Object(values))
            }
            _ => Err(Diagnostic::new(
                format!("这里需要一个表达式，但看到了 {}", self.current.describe()),
                self.current.line,
                self.current.column,
            )),
        }
    }

    fn parse_call_after_name(&mut self, name: String, allow_body: bool) -> Result<Expr> {
        let args = if matches!(self.current.kind, TokenKind::LParen) {
            self.parse_arguments()?
        } else {
            Vec::new()
        };

        let body = if allow_body
            && self.allow_block_body
            && matches!(self.current.kind, TokenKind::LBrace)
        {
            let base_name = name.split('.').next().unwrap_or(name.as_str());
            if TEXT_BLOCK_FNS.contains(&base_name) {
                if self.peeked.is_some() {
                    return Err(Diagnostic::new(
                        "内部错误：文本块前存在预读 token",
                        self.current.line,
                        self.current.column,
                    ));
                }
                self.lexer.enter_text();
                self.bump()?;
                Some(Block::Text(self.parse_text_block()?))
            } else {
                Some(Block::Code(self.parse_code_block()?))
            }
        } else {
            None
        };

        Ok(Expr::Call { name, args, body })
    }

    fn peek(&mut self) -> Result<&Token> {
        if self.peeked.is_none() {
            self.peeked = Some(self.lexer.next_token()?);
        }
        Ok(self.peeked.as_ref().unwrap())
    }

    fn bump(&mut self) -> Result<()> {
        if let Some(next) = self.peeked.take() {
            self.current = next;
        } else {
            self.current = self.lexer.next_token()?;
        }
        Ok(())
    }

    fn eat(&mut self, kind: TokenKind) -> Result<bool> {
        if self.same_variant(&self.current.kind, &kind) {
            self.bump()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn eat_semicolon(&mut self) -> Result<()> {
        if matches!(self.current.kind, TokenKind::Semicolon) {
            self.bump()?;
        }
        Ok(())
    }

    fn expect(&mut self, kind: TokenKind, expected: &str) -> Result<()> {
        if self.same_variant(&self.current.kind, &kind) {
            self.bump()?;
            Ok(())
        } else {
            Err(Diagnostic::new(
                format!("需要 {expected}，但看到了 {}", self.current.describe()),
                self.current.line,
                self.current.column,
            ))
        }
    }

    fn expect_ident(&mut self, expected: &str) -> Result<String> {
        match &self.current.kind {
            TokenKind::Ident(name) => {
                let name = name.clone();
                self.bump()?;
                Ok(name)
            }
            _ => Err(Diagnostic::new(
                format!("需要{expected}，但看到了 {}", self.current.describe()),
                self.current.line,
                self.current.column,
            )),
        }
    }

    fn expect_keyword(&mut self, keyword: &str) -> Result<()> {
        match &self.current.kind {
            TokenKind::Ident(name) if name == keyword => {
                self.bump()?;
                Ok(())
            }
            _ => Err(Diagnostic::new(
                format!(
                    "需要关键字 `{keyword}`，但看到了 {}",
                    self.current.describe()
                ),
                self.current.line,
                self.current.column,
            )),
        }
    }

    fn same_variant(&self, left: &TokenKind, right: &TokenKind) -> bool {
        std::mem::discriminant(left) == std::mem::discriminant(right)
    }
}
