#[derive(Debug, Clone)]
pub struct Program {
    pub stmts: Vec<Stmt>,
}

#[derive(Debug, Clone)]
pub enum Stmt {
    Let {
        name: String,
        value: Expr,
    },
    Fn {
        name: String,
        params: Vec<Param>,
        body: Vec<Stmt>,
    },
    Define {
        name: String,
        params: Vec<Param>,
        body: Vec<Stmt>,
    },
    Set {
        target: String,
        suffix: Option<String>,
        args: Vec<Argument>,
        body: Option<Vec<Stmt>>,
    },
    If {
        condition: Expr,
        then_branch: Vec<Stmt>,
        else_branch: Option<Vec<Stmt>>,
    },
    For {
        name: String,
        value_name: Option<String>,
        iterable: Expr,
        body: Vec<Stmt>,
    },
    Reference {
        name: String,
        args: Vec<Argument>,
        body: Option<Vec<Stmt>>,
    },
    Expr(Expr),
}

#[derive(Debug, Clone)]
pub struct Param {
    pub name: String,
    pub default: Option<Expr>,
}

#[derive(Debug, Clone)]
pub enum Expr {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Dimension(f64, String),
    Str(String),
    Ident(String),
    List(Vec<Expr>),
    Object(Vec<(String, Expr)>),
    Call {
        name: String,
        args: Vec<Argument>,
        body: Option<Block>,
    },
    Unary {
        op: UnaryOp,
        expr: Box<Expr>,
    },
    Binary {
        op: BinaryOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Range {
        start: Box<Expr>,
        end: Box<Expr>,
        inclusive: bool,
    },
    Index {
        target: Box<Expr>,
        index: Box<Expr>,
    },
    Member {
        target: Box<Expr>,
        name: String,
    },
}

#[derive(Debug, Clone)]
pub struct Argument {
    pub name: Option<String>,
    pub value: Expr,
}

#[derive(Debug, Clone)]
pub enum Block {
    Code(Vec<Stmt>),
    Text(Vec<TextPart>),
}

#[derive(Debug, Clone)]
pub enum TextPart {
    Text(String),
    Expr(Expr),
    Reference(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Not,
    Negate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Subtract,
    Multiply,
    Divide,
    Remainder,
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    And,
    Or,
}
