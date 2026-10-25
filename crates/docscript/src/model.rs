use crate::theme::{Style, Theme};

#[derive(Debug, Clone, PartialEq)]
pub struct Document {
    pub title: String,
    pub theme: Theme,
    pub blocks: Vec<Node>,
}

impl Document {
    pub fn new(title: impl Into<String>, theme: Theme, blocks: Vec<Node>) -> Self {
        Self {
            title: title.into(),
            theme,
            blocks,
        }
    }

    pub fn plain_text(&self) -> String {
        let mut out = String::new();
        for block in &self.blocks {
            block.write_plain_text(&mut out);
            out.push('\n');
        }
        out
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    Paragraph {
        inlines: Vec<Inline>,
        style: Option<Style>,
    },
    Heading {
        level: u8,
        inlines: Vec<Inline>,
        style: Option<Style>,
    },
    Quote {
        blocks: Vec<Node>,
        style: Option<Style>,
    },
    List {
        ordered: bool,
        items: Vec<ListItem>,
        style: Option<Style>,
    },
    ListItem {
        blocks: Vec<Node>,
        style: Option<Style>,
    },
    CodeBlock {
        lang: Option<String>,
        text: String,
        style: Option<Style>,
    },
    Divider {
        style: Option<Style>,
    },
    Figure {
        image: Image,
        caption: Vec<Inline>,
        style: Option<Style>,
    },
    Table {
        rows: Vec<TableRow>,
        style: Option<Style>,
    },
    TableRow {
        cells: Vec<TableCell>,
        style: Option<Style>,
    },
    TableCell {
        blocks: Vec<Node>,
        style: Option<Style>,
    },
    Bibliography {
        entries: Vec<ReferenceEntry>,
        style: Option<Style>,
    },
    Custom {
        name: String,
        blocks: Vec<Node>,
        style: Option<Style>,
    },
}

impl Node {
    pub fn paragraph(inlines: Vec<Inline>) -> Self {
        Self::Paragraph {
            inlines,
            style: None,
        }
    }

    pub fn style(&self) -> Option<&Style> {
        match self {
            Node::Paragraph { style, .. }
            | Node::Heading { style, .. }
            | Node::Quote { style, .. }
            | Node::List { style, .. }
            | Node::ListItem { style, .. }
            | Node::CodeBlock { style, .. }
            | Node::Divider { style }
            | Node::Figure { style, .. }
            | Node::Table { style, .. }
            | Node::TableRow { style, .. }
            | Node::TableCell { style, .. }
            | Node::Bibliography { style, .. }
            | Node::Custom { style, .. } => style.as_ref(),
        }
    }

    pub fn set_style(&mut self, value: Option<Style>) {
        match self {
            Node::Paragraph { style, .. }
            | Node::Heading { style, .. }
            | Node::Quote { style, .. }
            | Node::List { style, .. }
            | Node::ListItem { style, .. }
            | Node::CodeBlock { style, .. }
            | Node::Divider { style }
            | Node::Figure { style, .. }
            | Node::Table { style, .. }
            | Node::TableRow { style, .. }
            | Node::TableCell { style, .. }
            | Node::Bibliography { style, .. }
            | Node::Custom { style, .. } => *style = value,
        }
    }

    pub fn with_style(mut self, value: Option<Style>) -> Self {
        self.set_style(value);
        self
    }

    pub fn write_plain_text(&self, out: &mut String) {
        match self {
            Node::Paragraph { inlines, .. } | Node::Heading { inlines, .. } => {
                write_inlines_text(inlines, out)
            }
            Node::Quote { blocks, .. } => {
                out.push_str("> ");
                for block in blocks {
                    block.write_plain_text(out);
                }
            }
            Node::List { items, .. } => {
                for (index, item) in items.iter().enumerate() {
                    out.push_str(&format!("{}. ", index + 1));
                    item.write_plain_text(out);
                    out.push('\n');
                }
            }
            Node::ListItem { blocks, .. } => {
                for block in blocks {
                    block.write_plain_text(out);
                }
            }
            Node::CodeBlock { text, .. } => out.push_str(text),
            Node::Divider { .. } => out.push_str("---"),
            Node::Figure { image, caption, .. } => {
                out.push_str("[图片: ");
                out.push_str(&image.src);
                out.push(']');
                if !caption.is_empty() {
                    out.push('\n');
                    write_inlines_text(caption, out);
                }
            }
            Node::Table { rows, .. } => {
                for row in rows {
                    row.write_plain_text(out);
                    out.push('\n');
                }
            }
            Node::TableRow { cells, .. } => {
                for (index, cell) in cells.iter().enumerate() {
                    if index > 0 {
                        out.push_str(" | ");
                    }
                    cell.write_plain_text(out);
                }
            }
            Node::TableCell { blocks, .. } => {
                for block in blocks {
                    block.write_plain_text(out);
                }
            }
            Node::Bibliography { entries, .. } => {
                for (index, entry) in entries.iter().enumerate() {
                    out.push_str(&format!("[{}] ", index + 1));
                    write_inlines_text(&entry.inlines, out);
                    out.push('\n');
                }
            }
            Node::Custom { blocks, .. } => {
                for block in blocks {
                    block.write_plain_text(out);
                }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ListItem {
    pub blocks: Vec<Node>,
    pub style: Option<Style>,
}

impl ListItem {
    pub fn new(blocks: Vec<Node>) -> Self {
        Self {
            blocks,
            style: None,
        }
    }

    pub fn with_style(mut self, style: Option<Style>) -> Self {
        self.style = style;
        self
    }

    pub fn write_plain_text(&self, out: &mut String) {
        for block in &self.blocks {
            block.write_plain_text(out);
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TableRow {
    pub cells: Vec<TableCell>,
    pub style: Option<Style>,
}

impl TableRow {
    pub fn new(cells: Vec<TableCell>) -> Self {
        Self { cells, style: None }
    }

    pub fn with_style(mut self, style: Option<Style>) -> Self {
        self.style = style;
        self
    }

    pub fn write_plain_text(&self, out: &mut String) {
        for (index, cell) in self.cells.iter().enumerate() {
            if index > 0 {
                out.push_str(" | ");
            }
            cell.write_plain_text(out);
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TableCell {
    pub blocks: Vec<Node>,
    pub style: Option<Style>,
}

impl TableCell {
    pub fn new(blocks: Vec<Node>) -> Self {
        Self {
            blocks,
            style: None,
        }
    }

    pub fn with_style(mut self, style: Option<Style>) -> Self {
        self.style = style;
        self
    }

    pub fn write_plain_text(&self, out: &mut String) {
        for block in &self.blocks {
            block.write_plain_text(out);
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Inline {
    Text(String),
    Strong(Vec<Inline>),
    Emphasis(Vec<Inline>),
    Code(String),
    Link {
        target: String,
        children: Vec<Inline>,
    },
    Image(Image),
    Reference {
        index: usize,
    },
    Break,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Image {
    pub src: String,
    pub width: Option<ImageSize>,
    pub height: Option<ImageSize>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReferenceEntry {
    pub name: String,
    pub inlines: Vec<Inline>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ImageSize {
    Percent(f32),
    Points(f32),
}

impl Inline {
    pub fn plain_text(&self) -> String {
        let mut out = String::new();
        write_inline_text(self, &mut out);
        out
    }
}

fn write_inlines_text(inlines: &[Inline], out: &mut String) {
    for inline in inlines {
        write_inline_text(inline, out);
    }
}

fn write_inline_text(inline: &Inline, out: &mut String) {
    match inline {
        Inline::Text(text) | Inline::Code(text) => out.push_str(text),
        Inline::Strong(children) | Inline::Emphasis(children) => write_inlines_text(children, out),
        Inline::Link { children, .. } => write_inlines_text(children, out),
        Inline::Image(image) => {
            out.push_str("[图片: ");
            out.push_str(&image.src);
            out.push(']');
        }
        Inline::Reference { index } => {
            out.push_str(&format!("[{index}]"));
        }
        Inline::Break => out.push('\n'),
    }
}
