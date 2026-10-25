use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::rc::Rc;

use crate::ast::{Param, Stmt};
use crate::model::{Inline, Node};
use crate::theme::Theme;

#[derive(Clone)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Dimension(f32, String),
    Str(String),
    List(Rc<Vec<Value>>),
    Object(Rc<BTreeMap<String, Value>>),
    Inlines(Rc<Vec<Inline>>),
    Nodes(Rc<Vec<Node>>),
    Function(Rc<UserFunction>),
    Document(Rc<DocumentDraft>),
}

impl Value {
    pub fn list(values: Vec<Value>) -> Self {
        Value::List(Rc::new(values))
    }

    pub fn object(values: BTreeMap<String, Value>) -> Self {
        Value::Object(Rc::new(values))
    }

    pub fn inlines(values: Vec<Inline>) -> Self {
        Value::Inlines(Rc::new(values))
    }

    pub fn nodes(values: Vec<Node>) -> Self {
        Value::Nodes(Rc::new(values))
    }

    pub fn to_text(&self) -> String {
        match self {
            Value::Null => String::new(),
            Value::Bool(flag) => flag.to_string(),
            Value::Int(number) => number.to_string(),
            Value::Float(number) => {
                if number.fract() == 0.0 {
                    format!("{number:.0}")
                } else {
                    number.to_string()
                }
            }
            Value::Dimension(number, unit) => {
                if number.fract() == 0.0 {
                    format!("{number:.0}{unit}")
                } else {
                    format!("{number}{unit}")
                }
            }
            Value::Str(text) => text.clone(),
            Value::List(values) => values
                .iter()
                .map(Value::to_text)
                .collect::<Vec<_>>()
                .join(", "),
            Value::Object(_) => "[object]".to_string(),
            Value::Inlines(inlines) => collect_inline_text(inlines),
            Value::Nodes(nodes) => {
                let mut text = String::new();
                for node in nodes.iter() {
                    node.write_plain_text(&mut text);
                }
                text
            }
            Value::Function(function) => format!("<fn {}>", function.name),
            Value::Document(document) => document.title.clone(),
        }
    }
}

#[derive(Clone)]
pub struct DocumentDraft {
    pub title: String,
    pub theme: Theme,
    pub blocks: Vec<Node>,
}

#[derive(Clone)]
pub struct UserFunction {
    pub name: String,
    pub params: Vec<Param>,
    pub body: Vec<Stmt>,
    pub closure: Env,
    pub semantic: Option<String>,
}

#[derive(Clone, Default)]
pub struct Env(Rc<RefCell<EnvData>>);

#[derive(Default)]
struct EnvData {
    vars: HashMap<String, Value>,
    parent: Option<Env>,
}

impl Env {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn child(parent: &Env) -> Self {
        Self(Rc::new(RefCell::new(EnvData {
            vars: HashMap::new(),
            parent: Some(parent.clone()),
        })))
    }

    pub fn define(&self, name: impl Into<String>, value: Value) {
        self.0.borrow_mut().vars.insert(name.into(), value);
    }

    pub fn get(&self, name: &str) -> Option<Value> {
        let data = self.0.borrow();
        if let Some(value) = data.vars.get(name) {
            return Some(value.clone());
        }
        let parent = data.parent.clone();
        drop(data);
        parent.and_then(|parent| parent.get(name))
    }
}

pub fn collect_inline_text(inlines: &[Inline]) -> String {
    let mut out = String::new();
    for inline in inlines {
        out.push_str(&inline.plain_text());
    }
    out
}
