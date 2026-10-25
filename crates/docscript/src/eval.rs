use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::rc::Rc;

use crate::ast::*;
use crate::error::{Diagnostic, Result};
use crate::model::{
    Document, Image, ImageSize, Inline, ListItem, Node, ReferenceEntry, TableCell, TableRow,
};
use crate::parser::Parser;
use crate::theme::{PageStyle, Style, Theme};
use crate::value::{DocumentDraft, Env, UserFunction, Value, collect_inline_text};

pub fn evaluate(source: &str) -> std::result::Result<Document, Vec<Diagnostic>> {
    evaluate_with_base(source, None)
}

pub fn evaluate_with_base(
    source: &str,
    base_dir: Option<PathBuf>,
) -> std::result::Result<Document, Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let program = match Parser::new(source) {
        Ok(parser) => match parser.parse_program() {
            Ok(program) => program,
            Err(error) => {
                diagnostics.push(error);
                return Err(diagnostics);
            }
        },
        Err(error) => {
            diagnostics.push(error);
            return Err(diagnostics);
        }
    };

    match evaluate_program_with_base(&program, base_dir) {
        Ok(document) => Ok(document),
        Err(error) => {
            diagnostics.push(error);
            Err(diagnostics)
        }
    }
}

pub fn evaluate_program(program: &Program) -> Result<Document> {
    evaluate_program_with_base(program, None)
}

pub fn evaluate_program_with_base(
    program: &Program,
    base_dir: Option<PathBuf>,
) -> Result<Document> {
    let env = Env::new();
    let mut evaluator = Evaluator {
        base_dir,
        ..Evaluator::default()
    };
    let value = evaluator.eval_stmts(&program.stmts, &env)?;

    let bibliography_style = evaluator.node_style(NodeSelector::Bibliography);
    let bibliography = {
        let entries = std::mem::take(&mut evaluator.bibliography);
        if entries.is_empty() {
            None
        } else {
            Some(Node::Bibliography {
                entries,
                style: bibliography_style,
            })
        }
    };
    let page = evaluator.page_override.clone();
    if let Some(mut draft) = evaluator.document {
        if let Some(page) = page {
            draft.theme.page = page;
        }
        if let Some(bibliography) = bibliography.clone() {
            draft.blocks.push(bibliography);
        }
        Ok(Document::new(draft.title, draft.theme, draft.blocks))
    } else {
        let mut theme = Theme::default();
        if let Some(page) = page {
            theme.page = page;
        }
        let mut blocks = evaluator.nodes_from_value(value);
        if let Some(bibliography) = bibliography {
            blocks.push(bibliography);
        }
        Ok(Document::new("δ�����ĵ�", theme, blocks))
    }
}

#[derive(Default)]
struct Evaluator {
    document: Option<DocumentDraft>,
    styles: Vec<StyleRule>,
    classes: Vec<ClassRule>,
    page_override: Option<PageStyle>,
    base_dir: Option<PathBuf>,
    references: Vec<ReferenceEntry>,
    citation_index: HashMap<String, usize>,
    bibliography: Vec<ReferenceEntry>,
}

#[derive(Clone)]
struct StyleRule {
    selector: SetSelector,
    style: Style,
}

#[derive(Clone)]
struct ClassRule {
    selector: SetSelector,
    name: String,
    style: Style,
}

#[derive(Clone, PartialEq, Eq)]
enum SetSelector {
    Text,
    Page,
    Element {
        kind: ElementKind,
        level: Option<u8>,
    },
}

#[derive(Clone, PartialEq, Eq)]
enum ElementKind {
    Paragraph,
    Heading,
    Quote,
    List,
    Item,
    CodeBlock,
    Divider,
    Figure,
    Table,
    TableRow,
    TableCell,
    Bibliography,
    Custom(String),
}

#[derive(Clone, PartialEq, Eq)]
enum NodeSelector {
    Paragraph,
    Heading(u8),
    Quote,
    List,
    Item,
    CodeBlock,
    Divider,
    Figure,
    Table,
    TableRow,
    TableCell,
    Bibliography,
    Custom(String),
}

impl SetSelector {
    fn matches(&self, node: &NodeSelector) -> bool {
        match (self, node) {
            (SetSelector::Text, NodeSelector::Paragraph) => true,
            (
                SetSelector::Element {
                    kind: ElementKind::Paragraph,
                    ..
                },
                NodeSelector::Paragraph,
            ) => true,
            (
                SetSelector::Element {
                    kind: ElementKind::Heading,
                    level,
                },
                NodeSelector::Heading(node_level),
            ) => level.is_none_or(|level| level == *node_level),
            (
                SetSelector::Element {
                    kind: ElementKind::Quote,
                    ..
                },
                NodeSelector::Quote,
            ) => true,
            (
                SetSelector::Element {
                    kind: ElementKind::List,
                    ..
                },
                NodeSelector::List,
            ) => true,
            (
                SetSelector::Element {
                    kind: ElementKind::Item,
                    ..
                },
                NodeSelector::Item,
            ) => true,
            (
                SetSelector::Element {
                    kind: ElementKind::CodeBlock,
                    ..
                },
                NodeSelector::CodeBlock,
            ) => true,
            (
                SetSelector::Element {
                    kind: ElementKind::Divider,
                    ..
                },
                NodeSelector::Divider,
            ) => true,
            (
                SetSelector::Element {
                    kind: ElementKind::Figure,
                    ..
                },
                NodeSelector::Figure,
            ) => true,
            (
                SetSelector::Element {
                    kind: ElementKind::Table,
                    ..
                },
                NodeSelector::Table,
            ) => true,
            (
                SetSelector::Element {
                    kind: ElementKind::TableRow,
                    ..
                },
                NodeSelector::TableRow,
            ) => true,
            (
                SetSelector::Element {
                    kind: ElementKind::TableCell,
                    ..
                },
                NodeSelector::TableCell,
            ) => true,
            (
                SetSelector::Element {
                    kind: ElementKind::Bibliography,
                    ..
                },
                NodeSelector::Bibliography,
            ) => true,
            (
                SetSelector::Element {
                    kind: ElementKind::Custom(name),
                    ..
                },
                NodeSelector::Custom(node_name),
            ) => name == node_name,
            _ => false,
        }
    }
}

struct EvaluatedArgs {
    positional: Vec<Value>,
    named: std::collections::HashMap<String, Value>,
    body: Option<Value>,
}

impl Evaluator {
    fn eval_stmts(&mut self, stmts: &[Stmt], env: &Env) -> Result<Value> {
        let style_base = self.styles.len();
        let class_base = self.classes.len();
        let mut nodes = Vec::new();
        let mut pending: Vec<Inline> = Vec::new();
        let mut last = None;
        for stmt in stmts {
            match stmt {
                Stmt::Let { name, value } => {
                    let value = self.eval_expr(value, env)?;
                    env.define(name.clone(), value);
                }
                Stmt::Fn { name, params, body } => {
                    env.define(
                        name.clone(),
                        Value::Function(Rc::new(UserFunction {
                            name: name.clone(),
                            params: params.clone(),
                            body: body.clone(),
                            closure: env.clone(),
                            semantic: None,
                        })),
                    );
                }
                Stmt::Define { name, params, body } => {
                    env.define(
                        name.clone(),
                        Value::Function(Rc::new(UserFunction {
                            name: name.clone(),
                            params: params.clone(),
                            body: body.clone(),
                            closure: env.clone(),
                            semantic: Some(name.clone()),
                        })),
                    );
                }
                Stmt::Set {
                    target,
                    suffix,
                    args,
                    body,
                } => {
                    let value =
                        self.eval_set(target, suffix.as_deref(), args, body.as_deref(), env)?;
                    self.merge_value(value, &mut nodes, &mut pending, &mut last);
                }
                Stmt::If {
                    condition,
                    then_branch,
                    else_branch,
                } => {
                    let condition = self.eval_expr(condition, env)?;
                    let branch = if truthy(&condition) {
                        Some(then_branch.as_slice())
                    } else {
                        else_branch.as_deref()
                    };
                    if let Some(branch) = branch {
                        let child = Env::child(env);
                        let value = self.eval_stmts(branch, &child)?;
                        self.merge_value(value, &mut nodes, &mut pending, &mut last);
                    }
                }
                Stmt::For {
                    name,
                    value_name,
                    iterable,
                    body,
                } => {
                    let iterable = self.eval_expr(iterable, env)?;
                    let object_iter = matches!(&iterable, Value::Object(_));
                    for (key, item) in iterable_items(iterable) {
                        let child = Env::child(env);
                        let first = if value_name.is_some() || object_iter {
                            key
                        } else {
                            item.clone()
                        };
                        child.define(name.clone(), first);
                        if let Some(value_name) = value_name {
                            child.define(value_name.clone(), item);
                        }
                        let value = self.eval_stmts(body, &child)?;
                        self.merge_value(value, &mut nodes, &mut pending, &mut last);
                    }
                }
                Stmt::Reference { name, args, body } => {
                    let mut inlines = Vec::new();
                    for (arg_name, value) in self.eval_arguments(args, env)? {
                        if let Some(arg_name) = arg_name {
                            return Err(Diagnostic::new(
                                format!("@{name} 不支持命名参数 `{arg_name}`"),
                                1,
                                1,
                            ));
                        }
                        inlines.extend(inlines_from_value(value)?);
                    }
                    if let Some(body) = body {
                        let value = self.eval_stmts(body, &Env::child(env))?;
                        inlines.extend(inlines_from_value(value)?);
                    }
                    if inlines.is_empty() {
                        return Err(Diagnostic::new(format!("参考 @{name} 缺少内容"), 1, 1));
                    }
                    if self.references.iter().any(|entry| entry.name == *name) {
                        return Err(Diagnostic::new(format!("参考 @{name} 已定义"), 1, 1));
                    }
                    self.references.push(ReferenceEntry {
                        name: name.clone(),
                        inlines: normalize_inlines(inlines),
                    });
                }
                Stmt::Expr(expr) => {
                    let value = self.eval_expr(expr, env)?;
                    self.merge_value(value, &mut nodes, &mut pending, &mut last);
                }
            }
        }
        let value = self.finish_value(nodes, pending, last);
        self.styles.truncate(style_base);
        self.classes.truncate(class_base);
        Ok(value)
    }

    fn eval_set(
        &mut self,
        target: &str,
        suffix: Option<&str>,
        args: &[Argument],
        body: Option<&[Stmt]>,
        env: &Env,
    ) -> Result<Value> {
        let mut values = self.eval_arguments(args, env)?;
        let mut level = None;
        let mut properties: Vec<(String, Value)> = Vec::new();
        let class_name = suffix.filter(|name| *name != "style" && !is_style_property(name));

        if let Some(property) = suffix
            && class_name.is_none()
            && property != "style"
            && let Some(index) = values.iter().position(|(name, _)| name.is_none())
        {
            let (_, value) = values.remove(index);
            properties.push((property.to_string(), value));
        }

        for (name, value) in values {
            if target == "heading" && name.as_deref() == Some("level") {
                level = Some(as_int(&value)?.clamp(1, 6) as u8);
                continue;
            }
            match name.as_deref() {
                Some("style") => push_style_object(&mut properties, value)?,
                None if suffix == Some("style") || matches!(value, Value::Object(_)) => {
                    push_style_object(&mut properties, value)?
                }
                Some(name) => properties.push((name.to_string(), value)),
                None if target == "heading"
                    && level.is_none()
                    && matches!(value, Value::Int(_)) =>
                {
                    level = Some(as_int(&value)?.clamp(1, 6) as u8);
                }
                None => {
                    return Err(Diagnostic::new(
                        format!("@set {target} 不接受位置参数"),
                        1,
                        1,
                    ));
                }
            }
        }

        if target == "page" {
            if let Some(class_name) = class_name {
                return Err(Diagnostic::new(
                    format!("page 不支持类样式 `{class_name}`"),
                    1,
                    1,
                ));
            }
            let mut page = self.page_override.clone().unwrap_or_default();
            page.apply_pairs(properties)
                .map_err(|message| Diagnostic::new(message, 1, 1))?;
            self.page_override = Some(page);
            if let Some(body) = body {
                return self.eval_stmts(body, &Env::child(env));
            }
            return Ok(Value::Null);
        }

        let selector = set_selector(target, level);
        let style =
            Style::from_pairs(properties).map_err(|message| Diagnostic::new(message, 1, 1))?;

        if let Some(class_name) = class_name {
            let rule = ClassRule {
                selector,
                name: class_name.to_string(),
                style,
            };
            if let Some(body) = body {
                let class_base = self.classes.len();
                self.classes.push(rule);
                let value = self.eval_stmts(body, &Env::child(env))?;
                self.classes.truncate(class_base);
                Ok(value)
            } else {
                self.classes.push(rule);
                Ok(Value::Null)
            }
        } else if let Some(body) = body {
            let style_base = self.styles.len();
            self.styles.push(StyleRule { selector, style });
            let value = self.eval_stmts(body, &Env::child(env))?;
            self.styles.truncate(style_base);
            Ok(value)
        } else {
            if !style.is_empty() {
                self.styles.push(StyleRule { selector, style });
            }
            Ok(Value::Null)
        }
    }

    fn eval_arguments(
        &mut self,
        args: &[Argument],
        env: &Env,
    ) -> Result<Vec<(Option<String>, Value)>> {
        let mut values = Vec::new();
        for arg in args {
            values.push((arg.name.clone(), self.eval_expr(&arg.value, env)?));
        }
        Ok(values)
    }

    fn node_style(&self, selector: NodeSelector) -> Option<Style> {
        let mut style = Style::default();
        let mut matched = false;
        for rule in &self.styles {
            if rule.selector.matches(&selector) {
                style = style.merged(&rule.style);
                matched = true;
            }
        }
        (matched && !style.is_empty()).then_some(style)
    }

    fn styled(&self, mut node: Node, selector: NodeSelector) -> Node {
        node.set_style(self.node_style(selector));
        node
    }

    fn nodes_from_value(&self, value: Value) -> Vec<Node> {
        match value {
            Value::Nodes(nodes) => nodes.as_ref().clone(),
            Value::Inlines(inlines) => vec![self.styled(
                Node::paragraph(normalize_inlines(inlines.as_ref().clone())),
                NodeSelector::Paragraph,
            )],
            Value::Null => Vec::new(),
            Value::Str(text) => vec![self.styled(
                Node::paragraph(vec![Inline::Text(text)]),
                NodeSelector::Paragraph,
            )],
            Value::Bool(_) | Value::Int(_) | Value::Float(_) | Value::Dimension(_, _) => {
                vec![self.styled(
                    Node::paragraph(vec![Inline::Text(value.to_text())]),
                    NodeSelector::Paragraph,
                )]
            }
            _ => Vec::new(),
        }
    }

    fn finish_value(&self, nodes: Vec<Node>, pending: Vec<Inline>, last: Option<Value>) -> Value {
        if !pending.is_empty() {
            if nodes.is_empty() {
                return Value::inlines(pending);
            }
            let mut nodes = nodes;
            nodes.push(self.styled(
                Node::paragraph(normalize_inlines(pending)),
                NodeSelector::Paragraph,
            ));
            return Value::nodes(nodes);
        }
        if !nodes.is_empty() {
            Value::nodes(nodes)
        } else {
            last.unwrap_or(Value::Null)
        }
    }

    fn merge_value(
        &self,
        value: Value,
        nodes: &mut Vec<Node>,
        pending: &mut Vec<Inline>,
        last: &mut Option<Value>,
    ) {
        match value {
            Value::Nodes(values) => {
                if !pending.is_empty() {
                    nodes.push(self.styled(
                        Node::paragraph(normalize_inlines(std::mem::take(pending))),
                        NodeSelector::Paragraph,
                    ));
                }
                nodes.extend(values.iter().cloned());
            }
            Value::Inlines(values) => {
                pending.extend(values.iter().cloned());
                *last = Some(Value::inlines(values.as_ref().clone()));
            }
            Value::Str(text) => {
                pending.push(Inline::Text(text));
            }
            Value::Bool(_) | Value::Int(_) | Value::Float(_) | Value::Dimension(_, _) => {
                pending.push(Inline::Text(value.to_text()));
            }
            Value::Null | Value::List(_) | Value::Object(_) | Value::Function(_) => {}
            Value::Document(_) => {}
        }
    }

    fn eval_expr(&mut self, expr: &Expr, env: &Env) -> Result<Value> {
        match expr {
            Expr::Null => Ok(Value::Null),
            Expr::Bool(flag) => Ok(Value::Bool(*flag)),
            Expr::Int(number) => Ok(Value::Int(*number)),
            Expr::Float(number) => Ok(Value::Float(*number)),
            Expr::Dimension(number, unit) => Ok(Value::Dimension(*number as f32, unit.clone())),
            Expr::Str(text) => Ok(Value::Str(text.clone())),
            Expr::Ident(name) => env
                .get(name)
                .ok_or_else(|| Diagnostic::new(format!("未定义的变量 `{name}`"), 1, 1)),
            Expr::List(values) => {
                let values = values
                    .iter()
                    .map(|value| self.eval_expr(value, env))
                    .collect::<Result<Vec<_>>>()?;
                Ok(Value::list(values))
            }
            Expr::Object(values) => {
                let mut object = BTreeMap::new();
                for (key, value) in values {
                    object.insert(key.clone(), self.eval_expr(value, env)?);
                }
                Ok(Value::object(object))
            }
            Expr::Call { name, args, body } => self.eval_call(name, args, body, env),
            Expr::Unary { op, expr } => {
                let value = self.eval_expr(expr, env)?;
                match op {
                    UnaryOp::Not => Ok(Value::Bool(!truthy(&value))),
                    UnaryOp::Negate => match value {
                        Value::Int(number) => Ok(Value::Int(-number)),
                        Value::Float(number) => Ok(Value::Float(-number)),
                        other => Err(Diagnostic::new(
                            format!("不能对 {} 取负", other.to_text()),
                            1,
                            1,
                        )),
                    },
                }
            }
            Expr::Binary { op, left, right } => {
                if matches!(op, BinaryOp::And) {
                    let left = self.eval_expr(left, env)?;
                    if !truthy(&left) {
                        return Ok(Value::Bool(false));
                    }
                    let right = self.eval_expr(right, env)?;
                    return Ok(Value::Bool(truthy(&right)));
                }
                if matches!(op, BinaryOp::Or) {
                    let left = self.eval_expr(left, env)?;
                    if truthy(&left) {
                        return Ok(Value::Bool(true));
                    }
                    let right = self.eval_expr(right, env)?;
                    return Ok(Value::Bool(truthy(&right)));
                }
                let left = self.eval_expr(left, env)?;
                let right = self.eval_expr(right, env)?;
                eval_binary(*op, left, right)
            }
            Expr::Range {
                start,
                end,
                inclusive,
            } => {
                let start = as_int(&self.eval_expr(start, env)?)?;
                let end = as_int(&self.eval_expr(end, env)?)?;
                let end = if *inclusive { end + 1 } else { end };
                let values = if start <= end {
                    (start..end).map(Value::Int).collect()
                } else {
                    Vec::new()
                };
                Ok(Value::list(values))
            }
            Expr::Index { target, index } => {
                let target = self.eval_expr(target, env)?;
                let index = self.eval_expr(index, env)?;
                eval_index(target, index)
            }
            Expr::Member { target, name } => {
                let target = self.eval_expr(target, env)?;
                eval_member(target, name)
            }
        }
    }

    fn eval_call(
        &mut self,
        name: &str,
        args: &[Argument],
        body: &Option<Block>,
        env: &Env,
    ) -> Result<Value> {
        if let Some((base, class_name)) = name.split_once('.') {
            return self.call_class(base, class_name, args, body, env);
        }
        if let Some(Value::Function(function)) = env.get(name) {
            let args = self.eval_args(args, body, env)?;
            return self.call_user(&function, args);
        }

        let args = self.eval_args(args, body, env)?;
        self.call_builtin(name, args)
    }

    fn call_class(
        &mut self,
        base: &str,
        class_name: &str,
        args: &[Argument],
        body: &Option<Block>,
        env: &Env,
    ) -> Result<Value> {
        let value = self.eval_call(base, args, body, env)?;
        let Value::Nodes(nodes) = value else {
            return Err(Diagnostic::new(
                format!("类样式 `{base}.{class_name}` 只能用于块级元素"),
                1,
                1,
            ));
        };
        let mut nodes = nodes.as_ref().clone();
        let mut matched = false;
        for node in &mut nodes {
            let selector = node_selector(node);
            if let Some(class_style) = self.class_style(&selector, class_name) {
                let style = node
                    .style()
                    .cloned()
                    .unwrap_or_default()
                    .merged(&class_style);
                node.set_style(Some(style));
                matched = true;
            }
        }
        if !matched {
            return Err(Diagnostic::new(
                format!("类 `{class_name}` 未定义或不适用于 `{base}`"),
                1,
                1,
            ));
        }
        Ok(Value::nodes(nodes))
    }

    fn class_style(&self, selector: &NodeSelector, name: &str) -> Option<Style> {
        let mut style = Style::default();
        let mut matched = false;
        for rule in &self.classes {
            if rule.name == name && rule.selector.matches(selector) {
                style = style.merged(&rule.style);
                matched = true;
            }
        }
        matched.then_some(style)
    }

    fn eval_args(
        &mut self,
        args: &[Argument],
        body: &Option<Block>,
        env: &Env,
    ) -> Result<EvaluatedArgs> {
        let mut positional = Vec::new();
        let mut named = std::collections::HashMap::new();
        for arg in args {
            let value = self.eval_expr(&arg.value, env)?;
            match &arg.name {
                Some(name) => {
                    named.insert(name.clone(), value);
                }
                None => positional.push(value),
            }
        }

        let body = match body {
            Some(Block::Text(parts)) => Some(self.eval_text_parts(parts, env)?),
            Some(Block::Code(stmts)) => {
                let value = self.eval_stmts(stmts, &Env::child(env))?;
                Some(value)
            }
            None => None,
        };

        Ok(EvaluatedArgs {
            positional,
            named,
            body,
        })
    }

    fn eval_text_parts(&mut self, parts: &[TextPart], env: &Env) -> Result<Value> {
        let mut inlines: Vec<Inline> = Vec::new();
        let mut nodes: Vec<Node> = Vec::new();
        let mut has_blocks = false;

        for part in parts {
            match part {
                TextPart::Text(text) => inlines.push(Inline::Text(text.clone())),
                TextPart::Reference(name) => {
                    let index = self.cite_reference(name)?;
                    inlines.push(Inline::Reference { index });
                }
                TextPart::Expr(expr) => {
                    let value = self.eval_expr(expr, env)?;
                    match value {
                        Value::Nodes(values) => {
                            has_blocks = true;
                            let normalized = normalize_inlines(std::mem::take(&mut inlines));
                            if !normalized.is_empty() {
                                let paragraph = Node::paragraph(normalized);
                                nodes.push(self.styled(paragraph, NodeSelector::Paragraph));
                            }
                            nodes.extend(values.iter().cloned());
                        }
                        other => inlines.extend(inlines_from_value(other)?),
                    }
                }
            }
        }

        if has_blocks {
            let normalized = normalize_inlines(std::mem::take(&mut inlines));
            if !normalized.is_empty() {
                let paragraph = Node::paragraph(normalized);
                nodes.push(self.styled(paragraph, NodeSelector::Paragraph));
            }
            Ok(Value::nodes(nodes))
        } else {
            Ok(Value::inlines(normalize_inlines(inlines)))
        }
    }

    fn cite_reference(&mut self, name: &str) -> Result<usize> {
        if let Some(index) = self.citation_index.get(name) {
            return Ok(*index);
        }
        let entry = self
            .references
            .iter()
            .find(|entry| entry.name == name)
            .cloned()
            .ok_or_else(|| {
                Diagnostic::new(
                    format!("未定义的参考 @{name}，请先使用 @{name}(text {{ ... }}) 定义"),
                    1,
                    1,
                )
            })?;
        let index = self.bibliography.len() + 1;
        self.bibliography.push(entry);
        self.citation_index.insert(name.to_string(), index);
        Ok(index)
    }

    fn call_user(&mut self, function: &UserFunction, args: EvaluatedArgs) -> Result<Value> {
        let call_env = Env::child(&function.closure);
        let mut positional_index = 0;
        for param in &function.params {
            let value = if let Some(value) = args.named.get(&param.name) {
                value.clone()
            } else if let Some(value) = args.positional.get(positional_index) {
                positional_index += 1;
                value.clone()
            } else if let Some(default) = &param.default {
                self.eval_expr(default, &call_env)?
            } else {
                return Err(Diagnostic::new(
                    format!("函数 `{}` 缺少参数 `{}`", function.name, param.name),
                    1,
                    1,
                ));
            };
            call_env.define(param.name.clone(), value);
        }

        if let Some(body) = args.body {
            call_env.define("__body__", body);
        }

        let value = self.eval_stmts(&function.body, &call_env)?;
        if let Some(name) = &function.semantic {
            return Ok(self.custom_value(name, value));
        }
        let selector = NodeSelector::Custom(function.name.clone());
        if matches!(value, Value::Nodes(_)) && self.node_style(selector.clone()).is_some() {
            return Ok(self.custom_value(&function.name, value));
        }
        Ok(value)
    }

    fn custom_value(&self, name: &str, value: Value) -> Value {
        let blocks = self.nodes_from_value(value);
        let node = self.styled(
            Node::Custom {
                name: name.to_string(),
                blocks,
                style: None,
            },
            NodeSelector::Custom(name.to_string()),
        );
        Value::nodes(vec![node])
    }

    fn call_builtin(&mut self, name: &str, mut args: EvaluatedArgs) -> Result<Value> {
        match name {
            "document" => self.builtin_document(args),
            "p" | "paragraph" | "caption" | "label" => {
                let inlines = content_inlines(&mut args)?;
                let node = Node::Paragraph {
                    inlines,
                    style: None,
                };
                Ok(Value::nodes(vec![
                    self.styled(node, NodeSelector::Paragraph),
                ]))
            }
            "title" => {
                let inlines = content_inlines(&mut args)?;
                let node = Node::Heading {
                    level: 1,
                    inlines,
                    style: None,
                };
                Ok(Value::nodes(vec![
                    self.styled(node, NodeSelector::Heading(1)),
                ]))
            }
            "subtitle" => {
                let inlines = content_inlines(&mut args)?;
                let node = Node::Paragraph {
                    inlines,
                    style: None,
                };
                Ok(Value::nodes(vec![
                    self.styled(node, NodeSelector::Paragraph),
                ]))
            }
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                let level = name[1..].parse::<u8>().unwrap_or(1);
                let inlines = content_inlines(&mut args)?;
                let node = Node::Heading {
                    level,
                    inlines,
                    style: None,
                };
                Ok(Value::nodes(vec![
                    self.styled(node, NodeSelector::Heading(level)),
                ]))
            }
            "heading" => {
                let (level, rest) = split_level(&mut args);
                let inlines = content_inlines(&mut EvaluatedArgs {
                    positional: rest,
                    named: args.named,
                    body: args.body,
                })?;
                let node = Node::Heading {
                    level,
                    inlines,
                    style: None,
                };
                Ok(Value::nodes(vec![
                    self.styled(node, NodeSelector::Heading(level)),
                ]))
            }
            "figure" => self.builtin_figure(args),
            "table" => {
                let blocks = self.nodes_from_value(args.body.take().unwrap_or(Value::Null));
                let mut rows = Vec::new();
                for block in blocks {
                    match block {
                        Node::TableRow { cells, style } => {
                            rows.push(TableRow::new(cells).with_style(style))
                        }
                        other => rows.push(TableRow::new(vec![TableCell::new(vec![other])])),
                    }
                }
                let node = Node::Table { rows, style: None };
                Ok(Value::nodes(vec![self.styled(node, NodeSelector::Table)]))
            }
            "row" => {
                let blocks = self.nodes_from_value(args.body.take().unwrap_or(Value::Null));
                let mut cells = Vec::new();
                for block in blocks {
                    match block {
                        Node::TableCell { blocks, style } => {
                            cells.push(TableCell::new(blocks).with_style(style))
                        }
                        other => cells.push(TableCell::new(vec![other])),
                    }
                }
                let node = Node::TableRow { cells, style: None };
                Ok(Value::nodes(vec![
                    self.styled(node, NodeSelector::TableRow),
                ]))
            }
            "cell" => {
                let mut blocks = Vec::new();
                for value in args.positional.drain(..) {
                    blocks.extend(self.nodes_from_value(value));
                }
                if let Some(body) = args.body.take() {
                    blocks.extend(self.nodes_from_value(body));
                }
                let node = Node::TableCell {
                    blocks,
                    style: None,
                };
                Ok(Value::nodes(vec![
                    self.styled(node, NodeSelector::TableCell),
                ]))
            }
            "quote" => {
                let paragraph_style = self.node_style(NodeSelector::Paragraph);
                let blocks = container_blocks(&mut args, paragraph_style)?;
                let node = Node::Quote {
                    blocks,
                    style: None,
                };
                Ok(Value::nodes(vec![self.styled(node, NodeSelector::Quote)]))
            }
            "item" => {
                let paragraph_style = self.node_style(NodeSelector::Paragraph);
                let blocks = container_blocks(&mut args, paragraph_style)?;
                let node = Node::ListItem {
                    blocks,
                    style: None,
                };
                Ok(Value::nodes(vec![self.styled(node, NodeSelector::Item)]))
            }
            "list" => {
                let ordered = args
                    .named
                    .remove("ordered")
                    .map(|value| truthy(&value))
                    .unwrap_or(false);
                let blocks = self.nodes_from_value(args.body.take().unwrap_or(Value::Null));
                let mut items = Vec::new();
                for block in blocks {
                    match block {
                        Node::ListItem { blocks, style } => {
                            items.push(ListItem::new(blocks).with_style(style))
                        }
                        other => items.push(ListItem::new(vec![other])),
                    }
                }
                let node = Node::List {
                    ordered,
                    items,
                    style: None,
                };
                Ok(Value::nodes(vec![self.styled(node, NodeSelector::List)]))
            }
            "code_block" => {
                let lang = args
                    .named
                    .remove("lang")
                    .map(|value| value.to_text())
                    .or_else(|| {
                        if args.positional.len() > 1 && matches!(args.positional[1], Value::Str(_))
                        {
                            Some(args.positional[1].to_text())
                        } else {
                            None
                        }
                    });
                let text = args
                    .body
                    .take()
                    .map(inlines_text)
                    .or_else(|| args.positional.first().map(Value::to_text))
                    .unwrap_or_default();
                let node = Node::CodeBlock {
                    lang,
                    text,
                    style: None,
                };
                Ok(Value::nodes(vec![
                    self.styled(node, NodeSelector::CodeBlock),
                ]))
            }
            "hr" | "divider" => {
                let node = Node::Divider { style: None };
                Ok(Value::nodes(vec![self.styled(node, NodeSelector::Divider)]))
            }
            "text" | "span" => Ok(Value::inlines(content_inlines(&mut args)?)),
            "bold" | "strong" => {
                let inlines = content_inlines(&mut args)?;
                Ok(Value::inlines(vec![Inline::Strong(inlines)]))
            }
            "italic" | "em" => {
                let inlines = content_inlines(&mut args)?;
                Ok(Value::inlines(vec![Inline::Emphasis(inlines)]))
            }
            "code" => {
                let mut text = args
                    .positional
                    .iter()
                    .map(Value::to_text)
                    .collect::<Vec<_>>()
                    .join("");
                if let Some(body) = args.body.take() {
                    text.push_str(&inlines_text(body));
                }
                Ok(Value::inlines(vec![Inline::Code(text)]))
            }
            "link" => {
                let content = if let Some(value) = args.positional.first().cloned() {
                    inlines_from_value(value)?
                } else if let Some(body) = args.body.take() {
                    inlines_from_value(body)?
                } else {
                    Vec::new()
                };
                let target = args
                    .named
                    .remove("target")
                    .or_else(|| args.named.remove("to"))
                    .or_else(|| args.named.remove("href"))
                    .or_else(|| args.positional.get(1).cloned())
                    .map(|value| value.to_text())
                    .unwrap_or_else(|| collect_inline_text(&content));
                Ok(Value::inlines(vec![Inline::Link {
                    target,
                    children: content,
                }]))
            }
            "image" | "img" => self.builtin_image(args),
            "br" | "line_break" => Ok(Value::inlines(vec![Inline::Break])),
            "range" => {
                let start = args
                    .positional
                    .first()
                    .map(as_int)
                    .transpose()?
                    .unwrap_or(0);
                let end = if args.positional.len() > 1 {
                    as_int(&args.positional[1])?
                } else {
                    start
                };
                let start = if args.positional.len() > 1 { start } else { 0 };
                let values = (start..end).map(Value::Int).collect();
                Ok(Value::list(values))
            }
            "len" | "length" => {
                let value = args.positional.first().cloned().unwrap_or(Value::Null);
                let length = match value {
                    Value::Str(text) => text.chars().count() as i64,
                    Value::List(values) => values.len() as i64,
                    Value::Object(object) => object.len() as i64,
                    other => other.to_text().chars().count() as i64,
                };
                Ok(Value::Int(length))
            }
            "str" | "string" => Ok(Value::Str(
                args.positional
                    .iter()
                    .map(Value::to_text)
                    .collect::<Vec<_>>()
                    .join(""),
            )),
            "upper" => Ok(Value::Str(
                args.positional
                    .first()
                    .map(Value::to_text)
                    .unwrap_or_default()
                    .to_uppercase(),
            )),
            "lower" => Ok(Value::Str(
                args.positional
                    .first()
                    .map(Value::to_text)
                    .unwrap_or_default()
                    .to_lowercase(),
            )),
            "join" => {
                let separator = args
                    .positional
                    .get(1)
                    .map(Value::to_text)
                    .unwrap_or_default();
                let text = match args.positional.first() {
                    Some(Value::List(values)) => values
                        .iter()
                        .map(Value::to_text)
                        .collect::<Vec<_>>()
                        .join(&separator),
                    Some(other) => other.to_text(),
                    None => String::new(),
                };
                Ok(Value::Str(text))
            }
            "repeat" => {
                let count = args
                    .positional
                    .first()
                    .map(as_int)
                    .transpose()?
                    .unwrap_or(0);
                let text = args
                    .positional
                    .get(1)
                    .map(Value::to_text)
                    .or_else(|| args.body.take().map(inlines_text))
                    .unwrap_or_default();
                Ok(Value::Str(text.repeat(count.max(0) as usize)))
            }
            _ => Err(Diagnostic::new(
                format!("未知函数 `{name}`。可用函数请参考语法说明。"),
                1,
                1,
            )),
        }
    }

    fn builtin_image(&mut self, mut args: EvaluatedArgs) -> Result<Value> {
        if args.body.is_some() {
            return Err(Diagnostic::new("image 不接受花括号块", 1, 1));
        }
        let source = args
            .named
            .remove("src")
            .or_else(|| {
                if args.positional.is_empty() {
                    None
                } else {
                    Some(args.positional.remove(0))
                }
            })
            .ok_or_else(|| Diagnostic::new("image 缺少图片路径", 1, 1))?;
        let src = source.to_text().trim().to_string();
        if src.is_empty() {
            return Err(Diagnostic::new("image 的图片路径不能为空", 1, 1));
        }
        if !args.positional.is_empty() {
            return Err(Diagnostic::new("image 只接受一个位置参数：路径", 1, 1));
        }

        let width = args
            .named
            .remove("width")
            .map(|value| parse_image_size(value, "width"))
            .transpose()?;
        let height = args
            .named
            .remove("height")
            .map(|value| parse_image_size(value, "height"))
            .transpose()?;
        if let Some((name, _)) = args.named.into_iter().next() {
            return Err(Diagnostic::new(format!("image 不支持参数 `{name}`"), 1, 1));
        }

        let src = self.resolve_image_path(&src);
        Ok(Value::inlines(vec![Inline::Image(Image {
            src,
            width,
            height,
        })]))
    }

    fn resolve_image_path(&self, src: &str) -> String {
        let path = PathBuf::from(src);
        if path.is_absolute() {
            return path.to_string_lossy().into_owned();
        }
        match &self.base_dir {
            Some(base_dir) => base_dir.join(path).to_string_lossy().into_owned(),
            None => path.to_string_lossy().into_owned(),
        }
    }

    fn builtin_figure(&mut self, mut args: EvaluatedArgs) -> Result<Value> {
        let mut inlines = Vec::new();
        for value in args.positional.drain(..) {
            inlines.extend(inlines_from_value(value)?);
        }
        if let Some(body) = args.body.take() {
            inlines.extend(inlines_from_value(body)?);
        }
        if let Some((name, _)) = args.named.into_iter().next() {
            return Err(Diagnostic::new(format!("figure 不支持参数 `{name}`"), 1, 1));
        }

        let mut image = None;
        let mut caption = Vec::new();
        for inline in normalize_inlines(inlines) {
            match inline {
                Inline::Image(found) => {
                    if image.is_some() {
                        return Err(Diagnostic::new("figure 只能包含一张图片", 1, 1));
                    }
                    image = Some(found);
                }
                other => caption.push(other),
            }
        }
        let image =
            image.ok_or_else(|| Diagnostic::new("figure 需要一个 image(...) 图片", 1, 1))?;
        let node = Node::Figure {
            image,
            caption: normalize_inlines(caption),
            style: None,
        };
        Ok(Value::nodes(vec![self.styled(node, NodeSelector::Figure)]))
    }

    fn builtin_document(&mut self, mut args: EvaluatedArgs) -> Result<Value> {
        let mut theme = Theme::default();
        let mut title = "DocScript 文档".to_string();

        if let Some(value) = args.named.remove("theme") {
            theme = Theme::from_value(&value).map_err(|message| Diagnostic::new(message, 1, 1))?;
        }
        if let Some(value) = args
            .named
            .remove("title")
            .or_else(|| args.named.remove("name"))
        {
            title = value.to_text();
        }

        for value in args.positional {
            match value {
                Value::Object(_) => {
                    theme = Theme::from_value(&value)
                        .map_err(|message| Diagnostic::new(message, 1, 1))?;
                }
                Value::Str(text) => title = text,
                _ => {}
            }
        }

        let blocks = args
            .body
            .take()
            .map(|value| self.nodes_from_value(value))
            .unwrap_or_default();
        let draft = DocumentDraft {
            title: title.clone(),
            theme,
            blocks,
        };
        self.document = Some(draft.clone());
        Ok(Value::Document(Rc::new(draft)))
    }
}

fn push_style_object(properties: &mut Vec<(String, Value)>, value: Value) -> Result<()> {
    match value {
        Value::Object(object) => {
            for (key, value) in object.iter() {
                properties.push((key.clone(), value.clone()));
            }
            Ok(())
        }
        other => Err(Diagnostic::new(
            format!(
                "样式对象必须是 {{ size: 12, ... }}，但收到了 {}",
                other.to_text()
            ),
            1,
            1,
        )),
    }
}

fn parse_image_size(value: Value, name: &str) -> Result<ImageSize> {
    let (number, unit) = match value {
        Value::Dimension(number, unit) => (number, unit),
        Value::Int(number) => (number as f32, "px".to_string()),
        Value::Float(number) => (number as f32, "px".to_string()),
        Value::Str(text) => {
            let text = text.trim().to_lowercase();
            let split = text
                .find(|ch: char| !ch.is_ascii_digit() && ch != '.' && ch != '-')
                .unwrap_or(text.len());
            let (number, unit) = text.split_at(split);
            let unit = unit.trim().to_string();
            let number = number.parse::<f32>().map_err(|_| {
                Diagnostic::new(format!("image 的 `{name}` 不是有效尺寸：{text}"), 1, 1)
            })?;
            (number, unit.to_string())
        }
        other => {
            return Err(Diagnostic::new(
                format!(
                    "image 的 `{name}` 需要尺寸，例如 70%、300px、2cm；收到 {}",
                    other.to_text()
                ),
                1,
                1,
            ));
        }
    };

    if !number.is_finite() || number < 0.0 {
        return Err(Diagnostic::new(
            format!("image 的 `{name}` 必须是非负数字"),
            1,
            1,
        ));
    }

    let size = match unit.as_str() {
        "%" => ImageSize::Percent(number.min(1000.0)),
        "pt" => ImageSize::Points(number),
        "px" => ImageSize::Points(number * 0.75),
        "cm" => ImageSize::Points(number * 28.346_457),
        "mm" => ImageSize::Points(number * 2.834_646),
        "in" => ImageSize::Points(number * 72.0),
        "" => ImageSize::Points(number * 0.75),
        other => {
            return Err(Diagnostic::new(
                format!("image 的 `{name}` 不支持单位 `{other}`，可用 %、px、pt、cm、mm、in"),
                1,
                1,
            ));
        }
    };
    Ok(size)
}

fn is_style_property(name: &str) -> bool {
    matches!(
        name.trim().replace('_', "-").to_ascii_lowercase().as_str(),
        "font"
            | "font-family"
            | "size"
            | "font-size"
            | "weight"
            | "font-weight"
            | "italic"
            | "font-style"
            | "color"
            | "background"
            | "background-color"
            | "leading"
            | "line-height"
            | "align"
            | "text-align"
            | "indent"
            | "text-indent"
            | "margin"
            | "margin-before"
            | "margin-after"
            | "tracking"
            | "letter-spacing"
            | "style"
    )
}

fn node_selector(node: &Node) -> NodeSelector {
    match node {
        Node::Paragraph { .. } => NodeSelector::Paragraph,
        Node::Heading { level, .. } => NodeSelector::Heading(*level),
        Node::Quote { .. } => NodeSelector::Quote,
        Node::List { .. } => NodeSelector::List,
        Node::ListItem { .. } => NodeSelector::Item,
        Node::CodeBlock { .. } => NodeSelector::CodeBlock,
        Node::Divider { .. } => NodeSelector::Divider,
        Node::Figure { .. } => NodeSelector::Figure,
        Node::Table { .. } => NodeSelector::Table,
        Node::TableRow { .. } => NodeSelector::TableRow,
        Node::TableCell { .. } => NodeSelector::TableCell,
        Node::Bibliography { .. } => NodeSelector::Bibliography,
        Node::Custom { name, .. } => NodeSelector::Custom(name.clone()),
    }
}

fn set_selector(target: &str, level: Option<u8>) -> SetSelector {
    match target {
        "text" => SetSelector::Text,
        "page" => SetSelector::Page,
        "p" | "paragraph" | "caption" | "label" | "subtitle" => SetSelector::Element {
            kind: ElementKind::Paragraph,
            level: None,
        },
        "title" => SetSelector::Element {
            kind: ElementKind::Heading,
            level: Some(1),
        },
        "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
            let level = target[1..].parse::<u8>().unwrap_or(1);
            SetSelector::Element {
                kind: ElementKind::Heading,
                level: Some(level),
            }
        }
        "heading" => SetSelector::Element {
            kind: ElementKind::Heading,
            level,
        },
        "quote" => SetSelector::Element {
            kind: ElementKind::Quote,
            level: None,
        },
        "list" => SetSelector::Element {
            kind: ElementKind::List,
            level: None,
        },
        "item" => SetSelector::Element {
            kind: ElementKind::Item,
            level: None,
        },
        "code_block" => SetSelector::Element {
            kind: ElementKind::CodeBlock,
            level: None,
        },
        "hr" | "divider" => SetSelector::Element {
            kind: ElementKind::Divider,
            level: None,
        },
        "figure" => SetSelector::Element {
            kind: ElementKind::Figure,
            level: None,
        },
        "table" => SetSelector::Element {
            kind: ElementKind::Table,
            level: None,
        },
        "row" | "table_row" | "table-row" => SetSelector::Element {
            kind: ElementKind::TableRow,
            level: None,
        },
        "cell" | "table_cell" | "table-cell" => SetSelector::Element {
            kind: ElementKind::TableCell,
            level: None,
        },
        "bibliography" | "references" => SetSelector::Element {
            kind: ElementKind::Bibliography,
            level: None,
        },
        other => SetSelector::Element {
            kind: ElementKind::Custom(other.to_string()),
            level: None,
        },
    }
}

fn split_level(args: &mut EvaluatedArgs) -> (u8, Vec<Value>) {
    if let Some(level) = args.named.remove("level") {
        let level = as_int(&level).unwrap_or(1).clamp(1, 6) as u8;
        return (level, std::mem::take(&mut args.positional));
    }
    if let Some(Value::Int(level)) = args.positional.first().cloned() {
        let level = level.clamp(1, 6) as u8;
        let rest = args.positional.split_off(1);
        return (level, rest);
    }
    (1, std::mem::take(&mut args.positional))
}

fn content_inlines(args: &mut EvaluatedArgs) -> Result<Vec<Inline>> {
    let mut inlines = Vec::new();
    for value in args.positional.drain(..) {
        inlines.extend(inlines_from_value(value)?);
    }
    if let Some(body) = args.body.take() {
        inlines.extend(inlines_from_value(body)?);
    }
    Ok(normalize_inlines(inlines))
}

fn container_blocks(args: &mut EvaluatedArgs, paragraph_style: Option<Style>) -> Result<Vec<Node>> {
    let mut blocks = Vec::new();
    if let Some(body) = args.body.take() {
        match body {
            Value::Nodes(nodes) => blocks.extend(nodes.iter().cloned()),
            other => {
                let inlines = inlines_from_value(other)?;
                if !inlines.is_empty() {
                    blocks.push(Node::Paragraph {
                        inlines,
                        style: paragraph_style.clone(),
                    });
                }
            }
        }
    }
    if !args.positional.is_empty() || !args.named.is_empty() {
        let inlines = content_inlines(&mut EvaluatedArgs {
            positional: std::mem::take(&mut args.positional),
            named: std::mem::take(&mut args.named),
            body: None,
        })?;
        if !inlines.is_empty() {
            blocks.push(Node::Paragraph {
                inlines,
                style: paragraph_style.clone(),
            });
        }
    }
    if blocks.is_empty() {
        blocks.push(Node::Paragraph {
            inlines: Vec::new(),
            style: paragraph_style,
        });
    }
    Ok(blocks)
}

fn inlines_text(value: Value) -> String {
    match value {
        Value::Inlines(inlines) => collect_inline_text(&inlines),
        Value::Nodes(nodes) => {
            let mut text = String::new();
            for node in nodes.iter() {
                node.write_plain_text(&mut text);
                text.push('\n');
            }
            text
        }
        other => other.to_text(),
    }
}

fn inlines_from_value(value: Value) -> Result<Vec<Inline>> {
    match value {
        Value::Null => Ok(Vec::new()),
        Value::Str(text) => Ok(vec![Inline::Text(text)]),
        Value::Bool(_) | Value::Int(_) | Value::Float(_) | Value::Dimension(_, _) => {
            Ok(vec![Inline::Text(value.to_text())])
        }
        Value::Inlines(inlines) => Ok(inlines.as_ref().clone()),
        Value::Nodes(nodes) => {
            let mut inlines = Vec::new();
            for node in nodes.iter() {
                match node {
                    Node::Paragraph {
                        inlines: children, ..
                    }
                    | Node::Heading {
                        inlines: children, ..
                    } => inlines.extend(children.iter().cloned()),
                    other => {
                        let mut text = String::new();
                        other.write_plain_text(&mut text);
                        if !text.is_empty() {
                            inlines.push(Inline::Text(text));
                        }
                    }
                }
            }
            Ok(inlines)
        }
        Value::List(values) => Ok(vec![Inline::Text(
            values
                .iter()
                .map(Value::to_text)
                .collect::<Vec<_>>()
                .join(", "),
        )]),
        Value::Object(_) => Ok(vec![Inline::Text("[object]".to_string())]),
        Value::Function(function) => Ok(vec![Inline::Text(format!("<fn {}>", function.name))]),
        Value::Document(document) => Ok(vec![Inline::Text(document.title.clone())]),
    }
}

fn iterable_items(value: Value) -> Vec<(Value, Value)> {
    match value {
        Value::List(values) => values
            .iter()
            .enumerate()
            .map(|(index, value)| (Value::Int(index as i64), value.clone()))
            .collect(),
        Value::Str(text) => text
            .chars()
            .enumerate()
            .map(|(index, ch)| (Value::Int(index as i64), Value::Str(ch.to_string())))
            .collect(),
        Value::Int(end) => (0..end)
            .map(|index| (Value::Int(index), Value::Int(index)))
            .collect(),
        Value::Object(object) => object
            .iter()
            .map(|(key, value)| (Value::Str(key.clone()), value.clone()))
            .collect(),
        other => vec![(other.clone(), other)],
    }
}

fn eval_binary(op: BinaryOp, left: Value, right: Value) -> Result<Value> {
    match op {
        BinaryOp::Add => match (left, right) {
            (Value::Str(left), right) => Ok(Value::Str(left + &right.to_text())),
            (Value::Int(left), Value::Int(right)) => Ok(Value::Int(left + right)),
            (Value::Int(left), Value::Float(right)) => Ok(Value::Float(left as f64 + right)),
            (Value::Float(left), Value::Int(right)) => Ok(Value::Float(left + right as f64)),
            (Value::Float(left), Value::Float(right)) => Ok(Value::Float(left + right)),
            (Value::List(left), Value::List(right)) => {
                let mut values = left.as_ref().clone();
                values.extend(right.iter().cloned());
                Ok(Value::list(values))
            }
            (left, right) => Ok(Value::Str(left.to_text() + &right.to_text())),
        },
        BinaryOp::Subtract | BinaryOp::Multiply | BinaryOp::Divide | BinaryOp::Remainder => {
            let left = as_number(&left)?;
            let right = as_number(&right)?;
            let result = match op {
                BinaryOp::Subtract => left - right,
                BinaryOp::Multiply => left * right,
                BinaryOp::Divide => {
                    if right == 0.0 {
                        return Err(Diagnostic::new("不能除以 0", 1, 1));
                    }
                    left / right
                }
                BinaryOp::Remainder => {
                    if right == 0.0 {
                        return Err(Diagnostic::new("不能对 0 取余", 1, 1));
                    }
                    left % right
                }
                _ => unreachable!(),
            };
            Ok(Value::Float(result))
        }
        BinaryOp::Equal => Ok(Value::Bool(value_eq(&left, &right))),
        BinaryOp::NotEqual => Ok(Value::Bool(!value_eq(&left, &right))),
        BinaryOp::Less | BinaryOp::LessEqual | BinaryOp::Greater | BinaryOp::GreaterEqual => {
            let ordering = match (&left, &right) {
                (Value::Int(left), Value::Int(right)) => left.partial_cmp(right),
                (Value::Str(left), Value::Str(right)) => left.partial_cmp(right),
                _ => as_number(&left)?.partial_cmp(&as_number(&right)?),
            };
            let Some(ordering) = ordering else {
                return Ok(Value::Bool(false));
            };
            let result = match op {
                BinaryOp::Less => ordering.is_lt(),
                BinaryOp::LessEqual => ordering.is_le(),
                BinaryOp::Greater => ordering.is_gt(),
                BinaryOp::GreaterEqual => ordering.is_ge(),
                _ => unreachable!(),
            };
            Ok(Value::Bool(result))
        }
        BinaryOp::And | BinaryOp::Or => unreachable!(),
    }
}

fn eval_index(target: Value, index: Value) -> Result<Value> {
    match (target, index) {
        (Value::List(values), Value::Int(index)) => {
            let index = if index < 0 {
                values.len() as i64 + index
            } else {
                index
            };
            if index < 0 {
                return Err(Diagnostic::new("列表索引越界", 1, 1));
            }
            values
                .get(index as usize)
                .cloned()
                .ok_or_else(|| Diagnostic::new("列表索引越界", 1, 1))
        }
        (Value::Str(text), Value::Int(index)) => {
            let chars: Vec<char> = text.chars().collect();
            let index = if index < 0 {
                chars.len() as i64 + index
            } else {
                index
            };
            chars
                .get(index.max(0) as usize)
                .map(|ch| Value::Str(ch.to_string()))
                .ok_or_else(|| Diagnostic::new("字符串索引越界", 1, 1))
        }
        (Value::Object(object), Value::Str(key)) => object
            .get(&key)
            .cloned()
            .ok_or_else(|| Diagnostic::new(format!("对象没有字段 `{key}`"), 1, 1)),
        _ => Err(Diagnostic::new("不支持这种索引操作", 1, 1)),
    }
}

fn eval_member(target: Value, name: &str) -> Result<Value> {
    match &target {
        Value::Object(object) if name == "length" || name == "len" => {
            Ok(Value::Int(object.len() as i64))
        }
        Value::Object(object) => object
            .get(name)
            .cloned()
            .ok_or_else(|| Diagnostic::new(format!("对象没有字段 `{name}`"), 1, 1)),
        Value::List(values) if name == "length" || name == "len" => {
            Ok(Value::Int(values.len() as i64))
        }
        Value::Str(text) if name == "length" || name == "len" => {
            Ok(Value::Int(text.chars().count() as i64))
        }
        _ => Err(Diagnostic::new(
            format!("不能读取 {} 的字段 `{name}`", target.to_text()),
            1,
            1,
        )),
    }
}

fn truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(flag) => *flag,
        Value::Int(number) => *number != 0,
        Value::Float(number) => *number != 0.0,
        Value::Str(text) => !text.is_empty(),
        Value::List(values) => !values.is_empty(),
        Value::Inlines(values) => !values.is_empty(),
        Value::Nodes(values) => !values.is_empty(),
        _ => true,
    }
}

fn value_eq(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Null, Value::Null) => true,
        (Value::Bool(left), Value::Bool(right)) => left == right,
        (Value::Int(left), Value::Int(right)) => left == right,
        (Value::Float(left), Value::Float(right)) => left == right,
        (Value::Int(left), Value::Float(right)) => *left as f64 == *right,
        (Value::Float(left), Value::Int(right)) => *left == *right as f64,
        (Value::Str(left), Value::Str(right)) => left == right,
        (Value::List(left), Value::List(right)) => {
            left.len() == right.len()
                && left
                    .iter()
                    .zip(right.iter())
                    .all(|(left, right)| value_eq(left, right))
        }
        _ => false,
    }
}

fn as_number(value: &Value) -> Result<f64> {
    match value {
        Value::Int(number) => Ok(*number as f64),
        Value::Float(number) => Ok(*number),
        Value::Dimension(number, _) => Ok(*number as f64),
        _ => Err(Diagnostic::new("这里需要数字", 1, 1)),
    }
}

fn as_int(value: &Value) -> Result<i64> {
    match value {
        Value::Int(number) => Ok(*number),
        Value::Float(number) => Ok(*number as i64),
        Value::Dimension(number, _) => Ok(*number as i64),
        Value::Str(text) => text
            .parse::<i64>()
            .map_err(|_| Diagnostic::new("这里需要整数", 1, 1)),
        _ => Err(Diagnostic::new("这里需要整数", 1, 1)),
    }
}

pub fn normalize_inlines(inlines: Vec<Inline>) -> Vec<Inline> {
    let mut out = Vec::new();
    let mut pending_space = false;
    for inline in inlines {
        match inline {
            Inline::Text(text) => {
                for ch in text.chars() {
                    if ch.is_whitespace() {
                        pending_space = true;
                        continue;
                    }
                    if pending_space && !out.is_empty() {
                        let previous = last_char(&out);
                        if !(previous.is_some_and(is_cjk) && is_cjk(ch)) {
                            push_text(&mut out, " ");
                        }
                    }
                    pending_space = false;
                    let mut buffer = [0u8; 4];
                    push_text(&mut out, ch.encode_utf8(&mut buffer));
                }
            }
            Inline::Break => {
                pending_space = false;
                out.push(Inline::Break);
            }
            other => {
                let other = match other {
                    Inline::Strong(children) => Inline::Strong(normalize_inlines(children)),
                    Inline::Emphasis(children) => Inline::Emphasis(normalize_inlines(children)),
                    Inline::Link { target, children } => Inline::Link {
                        target,
                        children: normalize_inlines(children),
                    },
                    other => other,
                };
                if pending_space && !out.is_empty() {
                    let previous = last_char(&out);
                    let next = first_char(&other);
                    if !(previous.is_some_and(is_cjk) && next.is_some_and(is_cjk)) {
                        push_text(&mut out, " ");
                    }
                }
                pending_space = false;
                out.push(other);
            }
        }
    }
    out
}

fn push_text(out: &mut Vec<Inline>, text: &str) {
    if let Some(Inline::Text(existing)) = out.last_mut() {
        existing.push_str(text);
    } else {
        out.push(Inline::Text(text.to_string()));
    }
}

fn last_char(inlines: &[Inline]) -> Option<char> {
    inlines.last().and_then(inline_last_char)
}

fn first_char(inline: &Inline) -> Option<char> {
    match inline {
        Inline::Text(text) | Inline::Code(text) => text.chars().next(),
        Inline::Strong(children) | Inline::Emphasis(children) => {
            children.first().and_then(first_char)
        }
        Inline::Link { children, .. } => children.first().and_then(first_char),
        Inline::Image(_) | Inline::Reference { .. } | Inline::Break => None,
    }
}

fn inline_last_char(inline: &Inline) -> Option<char> {
    match inline {
        Inline::Text(text) | Inline::Code(text) => text.chars().last(),
        Inline::Strong(children) | Inline::Emphasis(children) => {
            children.last().and_then(inline_last_char)
        }
        Inline::Link { children, .. } => children.last().and_then(inline_last_char),
        Inline::Image(_) | Inline::Reference { .. } | Inline::Break => None,
    }
}

fn is_cjk(ch: char) -> bool {
    matches!(
        ch as u32,
        0x2E80..=0x9FFF
            | 0xF900..=0xFAFF
            | 0xFE30..=0xFE4F
            | 0xFF00..=0xFFEF
            | 0x20000..=0x2FA1F
    )
}
