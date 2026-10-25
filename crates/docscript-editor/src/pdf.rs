use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;

use crate::typeset::{self, Item};
use docscript::{
    Align, Document, Image, ImageSize, Inline, Node, Style, TableCell, TableRow, Theme,
};
use printpdf::{
    Codepoint, Color, FontId, Line, LinePoint, Mm, Op, PaintMode, ParsedFont, PdfDocument,
    PdfFontHandle, PdfPage, PdfSaveOptions, Point, Pt, RawImage, Rect, Rgb, TextItem, TextMatrix,
    XObjectId, XObjectTransform,
};
use rustybuzz::UnicodeBuffer;

const PT_PER_MM: f32 = 2.834_646;

pub fn render_pdf(document: &Document) -> Result<Vec<u8>, String> {
    let mut pdf = PdfDocument::new(&document.title);
    let fonts = Fonts::load(&mut pdf)?;
    let mut renderer = Renderer::new(document, fonts, pdf);
    renderer.render_blocks(&document.blocks, 0.0, None);
    renderer.finish();
    let (mut pdf, pages) = renderer.into_parts();

    let bytes = pdf
        .with_pages(pages)
        .save(&PdfSaveOptions::default(), &mut Vec::new());
    Ok(bytes)
}

struct FontFace {
    font: ParsedFont,
    bytes: Vec<u8>,
    index: usize,
    id: FontId,
}

impl FontFace {
    fn has(&self, ch: char) -> bool {
        self.font.lookup_glyph_index(ch as u32).is_some()
    }

    fn width(&self, ch: char, size: f32) -> Option<f32> {
        let gid = self.font.lookup_glyph_index(ch as u32)?;
        self.width_gid(gid, size)
    }

    fn width_gid(&self, gid: u16, size: f32) -> Option<f32> {
        let width = self.font.get_glyph_width(gid)?;
        let units = self.font.pdf_font_metrics.units_per_em.max(1) as f32;
        Some(width as f32 / units * size)
    }

    fn shape(&self, text: &str, size: f32) -> Result<Vec<RawGlyph>, String> {
        let face = rustybuzz::Face::from_slice(&self.bytes, self.index as u32)
            .ok_or_else(|| "HarfBuzz 无法解析字体".to_string())?;
        let units = face.units_per_em().max(1) as f32;
        let mut buffer = UnicodeBuffer::new();
        buffer.push_str(text);
        buffer.guess_segment_properties();
        let glyphs = rustybuzz::shape(&face, &[], buffer);
        let infos = glyphs.glyph_infos();
        let positions = glyphs.glyph_positions();
        let mut out = Vec::with_capacity(infos.len());
        for (info, position) in infos.iter().zip(positions.iter()) {
            out.push(RawGlyph {
                gid: info.glyph_id as u16,
                advance: position.x_advance as f32 / units * size,
                x_offset: position.x_offset as f32 / units * size,
                cluster: info.cluster as usize,
            });
        }
        Ok(out)
    }
}

struct Fonts {
    regular: FontFace,
    bold: FontFace,
    mono: FontFace,
}

impl Fonts {
    fn load(pdf: &mut PdfDocument) -> Result<Self, String> {
        let regular_font = load_parsed(
            "DOCSCRIPT_FONT",
            &[
                "C:/Windows/Fonts/msyh.ttc",
                "C:/Windows/Fonts/simhei.ttf",
                "C:/Windows/Fonts/Deng.ttf",
                "C:/Windows/Fonts/simsun.ttc",
                "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
                "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
                "/System/Library/Fonts/PingFang.ttc",
            ],
        )
        .ok_or_else(|| {
            "找不到可用的中文字体。请设置 DOCSCRIPT_FONT 指向一个 .ttf/.ttc 文件。".to_string()
        })?;

        let bold_font = load_parsed(
            "DOCSCRIPT_BOLD_FONT",
            &[
                "C:/Windows/Fonts/msyhbd.ttc",
                "C:/Windows/Fonts/simhei.ttf",
                "C:/Windows/Fonts/arialbd.ttf",
            ],
        )
        .unwrap_or_else(|| regular_font.clone());

        let mono_font = load_parsed(
            "DOCSCRIPT_MONO_FONT",
            &[
                "C:/Windows/Fonts/consola.ttf",
                "C:/Windows/Fonts/CascadiaMono.ttf",
                "C:/Windows/Fonts/CascadiaCode.ttf",
                "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
                "/System/Library/Fonts/Menlo.ttc",
            ],
        )
        .unwrap_or_else(|| regular_font.clone());

        let regular_id = pdf.add_font(&regular_font.parsed);
        let bold_id = pdf.add_font(&bold_font.parsed);
        let mono_id = pdf.add_font(&mono_font.parsed);

        Ok(Self {
            regular: FontFace {
                font: regular_font.parsed,
                bytes: regular_font.bytes,
                index: regular_font.index,
                id: regular_id,
            },
            bold: FontFace {
                font: bold_font.parsed,
                bytes: bold_font.bytes,
                index: bold_font.index,
                id: bold_id,
            },
            mono: FontFace {
                font: mono_font.parsed,
                bytes: mono_font.bytes,
                index: mono_font.index,
                id: mono_id,
            },
        })
    }

    fn face(&self, kind: FaceKind) -> &FontFace {
        match kind {
            FaceKind::Regular => &self.regular,
            FaceKind::Bold => &self.bold,
            FaceKind::Mono => &self.mono,
        }
    }

    fn face_for_char(&self, ch: char, requested: FaceKind) -> FaceKind {
        if self.face(requested).has(ch) {
            return requested;
        }
        if self.regular.has(ch) {
            return FaceKind::Regular;
        }
        if self.bold.has(ch) {
            return FaceKind::Bold;
        }
        if self.mono.has(ch) {
            return FaceKind::Mono;
        }
        FaceKind::Regular
    }
}

#[derive(Clone)]
struct LoadedFont {
    parsed: ParsedFont,
    bytes: Vec<u8>,
    index: usize,
}

fn load_parsed(env_key: &str, candidates: &[&str]) -> Option<LoadedFont> {
    let mut paths = Vec::new();
    if let Ok(path) = std::env::var(env_key)
        && !path.trim().is_empty()
    {
        paths.push(PathBuf::from(path));
    }
    for candidate in candidates {
        paths.push(PathBuf::from(candidate));
    }

    for path in paths {
        if !path.exists() {
            continue;
        }
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        let mut warnings = Vec::new();
        if let Some(font) = ParsedFont::from_bytes(&bytes, 0, &mut warnings) {
            return Some(LoadedFont {
                parsed: font,
                bytes,
                index: 0,
            });
        }
    }
    None
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FaceKind {
    Regular,
    Bold,
    Mono,
}

#[derive(Debug, Clone)]
struct StyledChar {
    ch: char,
    size: f32,
    color: String,
    face: FaceKind,
}

#[derive(Debug, Clone)]
struct RawGlyph {
    gid: u16,
    advance: f32,
    x_offset: f32,
    cluster: usize,
}

#[derive(Debug, Clone)]
struct ShapedGlyph {
    gid: u16,
    advance: f32,
    x_offset: f32,
    size: f32,
    color: String,
    face: FaceKind,
    cid: String,
}

struct ShapedRun {
    face: FaceKind,
    size: f32,
    color: String,
    start_mm: f32,
    items: Vec<TextItem>,
}

type Word = Vec<ShapedGlyph>;
type ShapedLine = typeset::Line<Word>;

#[derive(Clone)]
struct CellParagraph {
    inlines: Vec<Inline>,
    style: Style,
}

fn collect_cell_paragraphs(blocks: &[Node], base: &Style, theme: &Theme) -> Vec<CellParagraph> {
    let mut out = Vec::new();
    for block in blocks {
        collect_cell_node(block, base, theme, &mut out);
    }
    out
}

fn collect_cell_node(node: &Node, base: &Style, theme: &Theme, out: &mut Vec<CellParagraph>) {
    match node {
        Node::Paragraph { inlines, style } => {
            let mut resolved = base.clone();
            if let Some(style) = style {
                resolved = resolved.merged(style);
            }
            out.push(CellParagraph {
                inlines: inlines.clone(),
                style: resolved,
            });
        }
        Node::Heading { inlines, style, .. } => {
            let mut resolved = base.clone();
            if let Some(style) = style {
                resolved = resolved.merged(style);
            }
            out.push(CellParagraph {
                inlines: inlines.clone(),
                style: resolved,
            });
        }
        Node::Quote { blocks, style } => {
            let mut resolved = base.clone();
            if let Some(style) = style {
                resolved = resolved.merged(style);
            }
            for block in blocks {
                collect_cell_node(block, &resolved, theme, out);
            }
        }
        Node::List { ordered, items, .. } => {
            for (index, item) in items.iter().enumerate() {
                let mut item_style = base.clone();
                if let Some(style) = &item.style {
                    item_style = item_style.merged(style);
                }
                let mut inner = Vec::new();
                for block in &item.blocks {
                    collect_cell_node(block, &item_style, theme, &mut inner);
                }
                let marker = if *ordered {
                    format!("{}. ", index + 1)
                } else {
                    "• ".to_string()
                };
                if let Some(first) = inner.first_mut() {
                    let mut inlines = vec![Inline::Text(marker)];
                    inlines.append(&mut first.inlines);
                    first.inlines = inlines;
                    out.extend(inner);
                } else {
                    out.push(CellParagraph {
                        inlines: vec![Inline::Text(marker)],
                        style: item_style,
                    });
                }
            }
        }
        Node::ListItem { blocks, .. } => {
            for block in blocks {
                collect_cell_node(block, base, theme, out);
            }
        }
        Node::CodeBlock { text, style, .. } => {
            let mut resolved = theme.style("code_block").merged(&base.text_inheritance());
            if let Some(style) = style {
                resolved = resolved.merged(style);
            }
            for line in text.trim_end_matches('\n').split('\n') {
                out.push(CellParagraph {
                    inlines: vec![Inline::Text(line.to_string())],
                    style: resolved.clone(),
                });
            }
        }
        Node::Divider { .. } => {}
        Node::Figure { image, caption, .. } => {
            if caption.is_empty() {
                out.push(CellParagraph {
                    inlines: vec![Inline::Text(format!("[图片: {}]", image.src))],
                    style: base.clone(),
                });
            } else {
                out.push(CellParagraph {
                    inlines: caption.clone(),
                    style: base.clone(),
                });
            }
        }
        Node::Table { .. } => {
            let mut text = String::new();
            node.write_plain_text(&mut text);
            if !text.trim().is_empty() {
                out.push(CellParagraph {
                    inlines: vec![Inline::Text(text)],
                    style: base.clone(),
                });
            }
        }
        Node::TableRow { cells, .. } => {
            for cell in cells {
                let mut resolved = base.clone();
                if let Some(style) = &cell.style {
                    resolved = resolved.merged(style);
                }
                for block in &cell.blocks {
                    collect_cell_node(block, &resolved, theme, out);
                }
            }
        }
        Node::TableCell { blocks, style } => {
            let mut resolved = base.clone();
            if let Some(style) = style {
                resolved = resolved.merged(style);
            }
            for block in blocks {
                collect_cell_node(block, &resolved, theme, out);
            }
        }
        Node::Bibliography { entries, .. } => {
            for (index, entry) in entries.iter().enumerate() {
                let mut inlines = vec![Inline::Text(format!("[{}] ", index + 1))];
                inlines.extend(entry.inlines.iter().cloned());
                out.push(CellParagraph {
                    inlines,
                    style: base.clone(),
                });
            }
        }
        Node::Custom { blocks, style, .. } => {
            let mut resolved = base.clone();
            if let Some(style) = style {
                resolved = resolved.merged(style);
            }
            for block in blocks {
                collect_cell_node(block, &resolved, theme, out);
            }
        }
    }
}

#[derive(Clone)]
struct ImageAsset {
    id: XObjectId,
    width: usize,
    height: usize,
}

struct Renderer<'a> {
    theme: &'a Theme,
    fonts: Fonts,
    pdf: PdfDocument,
    images: HashMap<String, ImageAsset>,
    pages: Vec<PdfPage>,
    ops: Vec<Op>,
    width: f32,
    height: f32,
    margin: f32,
    y: f32,
    has_content: bool,
}

impl<'a> Renderer<'a> {
    fn new(document: &'a Document, fonts: Fonts, pdf: PdfDocument) -> Self {
        let page = &document.theme.page;
        Self {
            theme: &document.theme,
            fonts,
            pdf,
            images: HashMap::new(),
            pages: Vec::new(),
            ops: vec![Op::StartTextSection],
            width: page.width,
            height: page.height,
            margin: page.margin,
            y: page.height - page.margin,
            has_content: false,
        }
    }

    fn into_parts(self) -> (PdfDocument, Vec<PdfPage>) {
        (self.pdf, self.pages)
    }

    fn finish(&mut self) {
        if self.has_content || self.pages.is_empty() {
            self.finish_page();
        }
    }

    fn finish_page(&mut self) {
        self.ops.push(Op::EndTextSection);
        let page = PdfPage::new(
            Mm(self.width),
            Mm(self.height),
            std::mem::take(&mut self.ops),
        );
        self.pages.push(page);
        self.has_content = false;
    }

    fn start_page(&mut self) {
        self.ops = vec![Op::StartTextSection];
        self.y = self.height - self.margin;
        self.has_content = false;
    }

    fn ensure_space(&mut self, needed_mm: f32) {
        if self.has_content && self.y - needed_mm < self.margin {
            self.finish_page();
            self.start_page();
        }
    }

    fn space(&mut self, points: f32) {
        if self.has_content {
            self.y -= points / PT_PER_MM;
        }
    }

    fn content_width(&self, indent: f32) -> f32 {
        (self.width - self.margin * 2.0 - indent).max(20.0)
    }

    fn render_blocks(&mut self, blocks: &[Node], indent: f32, inherited: Option<&Style>) {
        for block in blocks {
            self.render_node(block, indent, inherited);
        }
    }

    fn render_node(&mut self, node: &Node, indent: f32, inherited: Option<&Style>) {
        match node {
            Node::Paragraph { inlines, style } => {
                let mut resolved = self.theme.style("text");
                if let Some(inherited) = inherited {
                    resolved = resolved.merged(&inherited.text_inheritance());
                }
                if let Some(style) = style {
                    resolved = resolved.merged(style);
                }
                self.render_paragraph(inlines, &resolved, indent);
            }
            Node::Heading {
                level,
                inlines,
                style,
            } => {
                let mut resolved = self.theme.heading_style(*level);
                if let Some(inherited) = inherited {
                    resolved = resolved.merged(&inherited.font_color_inheritance());
                }
                if let Some(style) = style {
                    resolved = resolved.merged(style);
                }
                self.space(resolved.margin_before_or(0.0));
                self.render_paragraph(inlines, &resolved, indent);
            }
            Node::Quote { blocks, style } => {
                let mut resolved = self.theme.style("quote");
                if let Some(style) = style {
                    resolved = resolved.merged(style);
                }
                self.space(resolved.margin_before_or(12.0));
                let child_indent = indent + resolved.indent_or(18.0) / PT_PER_MM;
                let child_style = resolved.text_inheritance();
                self.render_blocks(blocks, child_indent, Some(&child_style));
                self.space(resolved.margin_after_or(12.0));
            }
            Node::List {
                ordered,
                items,
                style,
            } => {
                let mut resolved = self.theme.style("list");
                if let Some(style) = style {
                    resolved = resolved.merged(style);
                }
                self.space(resolved.margin_before_or(8.0));
                let marker_indent = indent + resolved.indent_or(24.0) / PT_PER_MM;
                let child_style = resolved.text_inheritance();
                for (index, item) in items.iter().enumerate() {
                    let marker = if *ordered {
                        format!("{}. ", index + 1)
                    } else {
                        "• ".to_string()
                    };
                    self.render_list_item(item, &marker, marker_indent, Some(&child_style));
                    self.space(4.0);
                }
                self.space(resolved.margin_after_or(8.0));
            }
            Node::ListItem { blocks, style } => {
                let mut inherited = inherited.cloned();
                if let Some(style) = style {
                    inherited = Some(
                        inherited
                            .map(|inherited| inherited.merged(style))
                            .unwrap_or_else(|| style.clone()),
                    );
                }
                self.render_blocks(blocks, indent, inherited.as_ref());
            }
            Node::CodeBlock { text, style, .. } => {
                let mut resolved = self.theme.style("code_block");
                if let Some(style) = style {
                    resolved = resolved.merged(style);
                }
                self.space(resolved.margin_before_or(12.0));
                self.render_code_block(text, &resolved, indent);
                self.space(resolved.margin_after_or(12.0));
            }
            Node::Divider { style } => {
                let before = style
                    .as_ref()
                    .map(|style| style.margin_before_or(12.0))
                    .unwrap_or(12.0);
                let after = style
                    .as_ref()
                    .map(|style| style.margin_after_or(12.0))
                    .unwrap_or(12.0);
                self.space(before);
                self.ensure_space(0.5);
                self.draw_rule(indent);
                self.y -= after / PT_PER_MM;
                self.has_content = true;
            }
            Node::Figure {
                image,
                caption,
                style,
            } => {
                let mut resolved = self.theme.style("figure");
                if let Some(style) = style {
                    resolved = resolved.merged(style);
                }
                self.space(resolved.margin_before_or(16.0));
                self.render_image(image, &resolved, indent);
                if !caption.is_empty() {
                    let mut caption_style = self.theme.style("caption");
                    if let Some(align) = resolved.align {
                        caption_style.align = Some(align);
                    }
                    self.space(4.0);
                    self.render_text_lines(caption, &caption_style, indent);
                }
                self.space(resolved.margin_after_or(16.0));
            }
            Node::Table { rows, style } => {
                self.render_table(rows, style.as_ref(), indent);
            }
            Node::TableRow { cells, style } => {
                let row = TableRow::new(cells.clone()).with_style(style.clone());
                self.render_table(&[row], None, indent);
            }
            Node::TableCell { blocks, style } => {
                let cell = TableCell::new(blocks.clone()).with_style(style.clone());
                let row = TableRow::new(vec![cell]);
                self.render_table(&[row], None, indent);
            }
            Node::Bibliography { entries, style } => {
                let mut resolved = self.theme.style("bibliography");
                if let Some(style) = style {
                    resolved = resolved.merged(style);
                }
                self.space(resolved.margin_before_or(20.0));
                let title_style = self.theme.heading_style(2);
                self.ensure_space(0.5);
                self.draw_rule(indent);
                self.space(4.0);
                self.space(title_style.margin_before_or(0.0));
                self.render_text_lines(
                    &[Inline::Text("参考书目".to_string())],
                    &title_style,
                    indent,
                );
                self.space(6.0);
                let mut entry_style = self.theme.style("text");
                entry_style.size = Some(13.0);
                entry_style.leading = Some(1.5);
                entry_style.color = Some("#374151".to_string());
                for (index, entry) in entries.iter().enumerate() {
                    let mut inlines = vec![Inline::Text(format!("{}. ", index + 1))];
                    inlines.extend(entry.inlines.iter().cloned());
                    self.render_text_lines(&inlines, &entry_style, indent);
                    self.space(4.0);
                }
                self.space(resolved.margin_after_or(12.0));
            }
            Node::Custom {
                name,
                blocks,
                style,
            } => {
                let mut resolved = self.theme.style(name);
                if let Some(style) = style {
                    resolved = resolved.merged(style);
                }
                self.space(resolved.margin_before_or(10.0));
                let child_indent = indent + resolved.indent_or(14.0) / PT_PER_MM;
                let child_style = resolved.text_inheritance();
                self.render_blocks(blocks, child_indent, Some(&child_style));
                self.space(resolved.margin_after_or(10.0));
            }
        }
    }

    fn render_table(&mut self, rows: &[TableRow], style: Option<&Style>, indent: f32) {
        if rows.is_empty() {
            return;
        }
        let mut table_style = self.theme.style("table");
        if let Some(style) = style {
            table_style = table_style.merged(style);
        }
        let margin_before = table_style.margin_before.unwrap_or(12.0);
        let margin_after = table_style.margin_after.unwrap_or(12.0);
        self.space(margin_before);

        let columns = rows
            .iter()
            .map(|row| row.cells.len())
            .max()
            .unwrap_or(0)
            .max(1);
        let content_width = self.content_width(indent);
        let column_width = content_width / columns as f32;
        let padding = 4.0 / PT_PER_MM;
        let inner_width = (column_width - padding * 2.0).max(4.0);
        let inner_width_pt = inner_width * PT_PER_MM;

        for row in rows {
            let mut row_style = self
                .theme
                .style("row")
                .merged(&table_style.text_inheritance());
            if let Some(style) = &row.style {
                row_style = row_style.merged(style);
            }

            let mut cell_styles = Vec::with_capacity(columns);
            let mut cell_paragraphs = Vec::with_capacity(columns);
            let mut cell_heights = Vec::with_capacity(columns);
            for column in 0..columns {
                let cell = row.cells.get(column);
                let mut cell_style = self
                    .theme
                    .style("cell")
                    .merged(&row_style.text_inheritance());
                if let Some(cell) = cell
                    && let Some(style) = &cell.style
                {
                    cell_style = cell_style.merged(style);
                }
                if cell_style.background.is_none() {
                    cell_style.background = row_style
                        .background
                        .clone()
                        .or_else(|| table_style.background.clone());
                }
                let paragraphs = cell
                    .map(|cell| collect_cell_paragraphs(&cell.blocks, &cell_style, self.theme))
                    .unwrap_or_default();
                let mut height = padding * 2.0;
                for paragraph in &paragraphs {
                    height += self.cell_paragraph_height(paragraph, inner_width_pt);
                }
                cell_styles.push(cell_style);
                cell_paragraphs.push(paragraphs);
                cell_heights.push(height.max(1.0));
            }

            let row_height = cell_heights
                .iter()
                .copied()
                .fold(padding * 2.0 + 4.0, f32::max);
            self.ensure_space(row_height + 1.0);
            let row_top = self.y;
            let row_bottom = row_top - row_height;

            self.ops.push(Op::EndTextSection);
            for (column, cell_style) in cell_styles.iter().enumerate() {
                let x = self.margin + indent + column as f32 * column_width;
                let rect = Rect {
                    x: Pt(x * PT_PER_MM),
                    y: Pt(row_bottom * PT_PER_MM),
                    width: Pt(column_width * PT_PER_MM),
                    height: Pt(row_height * PT_PER_MM),
                    mode: Some(PaintMode::Fill),
                    winding_order: None,
                };
                if let Some(background) = &cell_style.background {
                    self.ops.push(Op::SetFillColor {
                        col: pdf_color(background),
                    });
                    self.ops.push(Op::DrawRectangle {
                        rectangle: rect.clone(),
                    });
                }
                self.ops.push(Op::SetOutlineColor {
                    col: pdf_color("#d8dee4"),
                });
                self.ops.push(Op::SetOutlineThickness { pt: Pt(0.5) });
                self.ops.push(Op::DrawRectangle {
                    rectangle: Rect {
                        mode: Some(PaintMode::Stroke),
                        ..rect
                    },
                });
            }
            self.ops.push(Op::StartTextSection);

            for (column, paragraphs) in cell_paragraphs.iter().enumerate() {
                let x = self.margin + indent + column as f32 * column_width + padding;
                let mut cell_y = row_top - padding;
                for paragraph in paragraphs {
                    let chars = collect_inlines(&paragraph.inlines, &paragraph.style, self.theme);
                    if chars.is_empty() {
                        continue;
                    }
                    let line_height =
                        paragraph.style.size_or(15.0) * paragraph.style.leading_or(1.5) / PT_PER_MM;
                    let align = paragraph.style.align_or(Align::Left);
                    match self.shape_and_break(&chars, inner_width_pt) {
                        Ok(lines) if !lines.is_empty() => {
                            for line in lines {
                                let max_size = line
                                    .items
                                    .iter()
                                    .filter_map(|item| match item {
                                        Item::Box(word, _) => {
                                            word.iter().map(|glyph| glyph.size).reduce(f32::max)
                                        }
                                        _ => None,
                                    })
                                    .fold(0.0_f32, f32::max)
                                    .max(1.0);
                                let baseline = cell_y - max_size * 0.8 / PT_PER_MM;
                                self.draw_shaped_line_at(&line, x, baseline, align, inner_width_pt);
                                cell_y -= line_height;
                            }
                        }
                        _ => {
                            for line in self.layout(&chars, inner_width_pt) {
                                let max_size = line
                                    .iter()
                                    .map(|ch| ch.size)
                                    .fold(0.0_f32, f32::max)
                                    .max(1.0);
                                let baseline = cell_y - max_size * 0.8 / PT_PER_MM;
                                self.draw_line_at(&line, x, baseline, align, inner_width_pt);
                                cell_y -= line_height;
                            }
                        }
                    }
                    cell_y -= 1.5 / PT_PER_MM;
                }
            }

            self.y = row_bottom - 1.0 / PT_PER_MM;
            self.has_content = true;
        }

        self.space(margin_after);
    }

    fn cell_paragraph_height(&self, paragraph: &CellParagraph, width_pt: f32) -> f32 {
        let chars = collect_inlines(&paragraph.inlines, &paragraph.style, self.theme);
        if chars.is_empty() {
            return 0.0;
        }
        let line_height =
            paragraph.style.size_or(15.0) * paragraph.style.leading_or(1.5) / PT_PER_MM;
        let lines = match self.shape_and_break(&chars, width_pt) {
            Ok(lines) if !lines.is_empty() => lines.len(),
            _ => self.layout(&chars, width_pt).len(),
        };
        lines as f32 * line_height + 1.5 / PT_PER_MM
    }

    fn render_list_item(
        &mut self,
        item: &docscript::ListItem,
        marker: &str,
        indent: f32,
        inherited: Option<&Style>,
    ) {
        let mut inherited = inherited.cloned();
        if let Some(style) = &item.style {
            inherited = Some(
                inherited
                    .map(|inherited| inherited.merged(style))
                    .unwrap_or_else(|| style.clone()),
            );
        }
        let mut style = self.theme.style("text");
        if let Some(inherited) = inherited.as_ref() {
            style = style.merged(&inherited.text_inheritance());
        }
        let mut blocks = item.blocks.iter();
        if let Some(Node::Paragraph {
            inlines,
            style: paragraph_style,
        }) = blocks.clone().next()
        {
            let mut resolved = style.clone();
            if let Some(paragraph_style) = paragraph_style {
                resolved = resolved.merged(paragraph_style);
            }
            let mut prefixed = Vec::with_capacity(inlines.len() + 1);
            prefixed.push(Inline::Text(marker.to_string()));
            prefixed.extend(inlines.iter().cloned());
            self.render_paragraph(&prefixed, &resolved, indent);
            blocks.next();
        } else {
            self.render_paragraph(&[Inline::Text(marker.to_string())], &style, indent);
        }
        for block in blocks {
            self.render_node(block, indent, inherited.as_ref());
        }
    }

    fn render_paragraph(&mut self, inlines: &[Inline], style: &Style, indent: f32) {
        if inlines
            .iter()
            .any(|inline| matches!(inline, Inline::Image(_)))
        {
            let mut segment = Vec::new();
            for inline in inlines {
                if let Inline::Image(image) = inline {
                    if !segment.is_empty() {
                        self.render_text_lines(&segment, style, indent);
                        segment.clear();
                    }
                    self.render_image(image, style, indent);
                } else {
                    segment.push(inline.clone());
                }
            }
            if !segment.is_empty() {
                self.render_text_lines(&segment, style, indent);
            }
        } else {
            self.render_text_lines(inlines, style, indent);
        }
        self.space(style.margin_after_or(10.0));
    }

    fn render_text_lines(&mut self, inlines: &[Inline], style: &Style, indent: f32) {
        let chars = collect_inlines(inlines, style, self.theme);
        if chars.is_empty() {
            return;
        }
        let size = style.size_or(15.0);
        let line_height = size * style.leading_or(1.75) / PT_PER_MM;
        let max_width_pt = self.content_width(indent) * PT_PER_MM;

        match self.shape_and_break(&chars, max_width_pt) {
            Ok(lines) if !lines.is_empty() => {
                for line in lines {
                    self.draw_shaped_line(
                        &line,
                        indent,
                        style.align_or(Align::Left),
                        max_width_pt,
                        line_height,
                    );
                }
            }
            _ => {
                // Shaping is unavailable for this font: keep text visible with
                // the simpler metric-based fallback.
                let lines = self.layout(&chars, max_width_pt);
                for line in lines {
                    self.draw_line(
                        &line,
                        indent,
                        style.align_or(Align::Left),
                        max_width_pt,
                        line_height,
                    );
                }
            }
        }
    }

    fn render_image(&mut self, image: &Image, style: &Style, indent: f32) {
        let asset = match self.load_image(&image.src) {
            Ok(asset) => asset,
            Err(error) => {
                self.render_text_lines(
                    &[Inline::Text(format!("[图片读取失败: {error}]"))],
                    style,
                    indent,
                );
                return;
            }
        };
        let content_width = self.content_width(indent);
        let max_height = (self.height - self.margin * 2.0 - 6.0).max(20.0);
        let (width_mm, height_mm) =
            image_dimensions(image, asset.width, asset.height, content_width, max_height);
        self.space(6.0);
        self.ensure_space(height_mm + 4.0);
        let align_offset = match style.align_or(Align::Left) {
            Align::Center => ((content_width - width_mm) / 2.0).max(0.0),
            Align::Right => (content_width - width_mm).max(0.0),
            Align::Left | Align::Justify => 0.0,
        };
        let x_mm = self.margin + indent + align_offset;
        let bottom_mm = self.y - height_mm;
        let width_pt = width_mm * PT_PER_MM;
        let dpi = asset.width as f32 * 72.0 / width_pt.max(0.01);
        self.ops.push(Op::EndTextSection);
        self.ops.push(Op::UseXobject {
            id: asset.id,
            transform: XObjectTransform {
                translate_x: Some(Pt(x_mm * PT_PER_MM)),
                translate_y: Some(Pt(bottom_mm * PT_PER_MM)),
                dpi: Some(dpi),
                ..XObjectTransform::default()
            },
        });
        self.ops.push(Op::StartTextSection);
        self.y = bottom_mm - 4.0 / PT_PER_MM;
        self.has_content = true;
    }

    fn load_image(&mut self, src: &str) -> Result<ImageAsset, String> {
        if let Some(asset) = self.images.get(src) {
            return Ok(asset.clone());
        }
        let path = image_path(src);
        let bytes =
            std::fs::read(&path).map_err(|error| format!("{}（{error}）", path.display()))?;
        let mut warnings = Vec::new();
        let raw = RawImage::decode_from_bytes(&bytes, &mut warnings)
            .map_err(|error| format!("{}（{error}）", path.display()))?;
        let asset = ImageAsset {
            width: raw.width,
            height: raw.height,
            id: self.pdf.add_image(&raw),
        };
        self.images.insert(src.to_string(), asset.clone());
        Ok(asset)
    }

    fn shape_and_break(
        &self,
        chars: &[StyledChar],
        line_width_pt: f32,
    ) -> Result<Vec<ShapedLine>, String> {
        let plain: String = chars.iter().map(|ch| ch.ch).collect();
        let breaks: BTreeSet<usize> = unicode_linebreak::linebreaks(&plain)
            .map(|(offset, _)| offset)
            .collect();
        let mut byte_offsets = Vec::with_capacity(chars.len());
        let mut offset = 0;
        for ch in chars {
            let end = offset + ch.ch.len_utf8();
            byte_offsets.push((offset, end));
            offset = end;
        }

        let mut lines = Vec::new();
        let mut segment_start = 0;
        for segment_end in 0..=chars.len() {
            if segment_end < chars.len() && chars[segment_end].ch != '\n' {
                continue;
            }

            let segment = &chars[segment_start..segment_end];
            if segment.is_empty() {
                lines.push(ShapedLine {
                    items: Vec::new(),
                    width: 0.0,
                    ratio: 0.0,
                });
                segment_start = segment_end + 1;
                continue;
            }

            let segment_byte_end = byte_offsets[segment_end - 1].1;
            let mut items: Vec<Item<Word>> = vec![Item::Penalty {
                width: 0.0,
                penalty: -10_000.0,
            }];
            let mut word: Word = Vec::new();
            let mut word_width = 0.0;
            let mut char_index = segment_start;

            while char_index < segment_end {
                let first = &chars[char_index];
                let face = self.fonts.face_for_char(first.ch, first.face);
                let mut run_end = char_index + 1;
                while run_end < segment_end {
                    let candidate = &chars[run_end];
                    if candidate.size != first.size
                        || candidate.color != first.color
                        || self.fonts.face_for_char(candidate.ch, candidate.face) != face
                    {
                        break;
                    }
                    run_end += 1;
                }

                let run_text: String = chars[char_index..run_end].iter().map(|ch| ch.ch).collect();
                let run_byte_start = byte_offsets[char_index].0;
                let run_byte_end = byte_offsets[run_end - 1].1;
                let raw = self.fonts.face(face).shape(&run_text, first.size)?;
                let mut last_cid_cluster = usize::MAX;

                for (glyph_index, glyph) in raw.iter().enumerate() {
                    let cluster = run_byte_start + glyph.cluster;
                    let cluster_end = if glyph_index + 1 < raw.len() {
                        let next = run_byte_start + raw[glyph_index + 1].cluster;
                        if next > cluster { next } else { run_byte_end }
                    } else {
                        run_byte_end
                    };
                    let source_index = byte_offsets
                        .partition_point(|(start, _)| *start <= cluster)
                        .saturating_sub(1)
                        .min(chars.len().saturating_sub(1));
                    let source = chars[source_index].ch;
                    let is_space = source.is_whitespace();
                    let break_after =
                        cluster_end < segment_byte_end && breaks.contains(&cluster_end);
                    let cid = if cluster != last_cid_cluster {
                        last_cid_cluster = cluster;
                        plain.get(cluster..cluster_end).unwrap_or("").to_string()
                    } else {
                        String::new()
                    };
                    let shaped = ShapedGlyph {
                        gid: glyph.gid,
                        advance: glyph.advance,
                        x_offset: glyph.x_offset,
                        size: first.size,
                        color: first.color.clone(),
                        face,
                        cid,
                    };

                    if is_space {
                        flush_word(&mut items, &mut word, &mut word_width, line_width_pt);
                        items.push(Item::Glue {
                            width: shaped.advance,
                            stretch: first.size * 0.5,
                            shrink: first.size * 0.3,
                        });
                        if break_after {
                            items.push(Item::Penalty {
                                width: 0.0,
                                penalty: 0.0,
                            });
                        }
                    } else {
                        word_width += shaped.advance;
                        word.push(shaped);
                        if break_after {
                            flush_word(&mut items, &mut word, &mut word_width, line_width_pt);
                            items.push(Item::Penalty {
                                width: 0.0,
                                penalty: 0.0,
                            });
                        }
                    }
                }

                char_index = run_end;
            }

            flush_word(&mut items, &mut word, &mut word_width, line_width_pt);
            items.push(Item::Penalty {
                width: 0.0,
                penalty: -10_000.0,
            });
            lines.extend(typeset::break_lines(&items, line_width_pt));
            segment_start = segment_end + 1;
        }

        Ok(lines)
    }

    fn draw_shaped_line(
        &mut self,
        line: &ShapedLine,
        indent: f32,
        align: Align,
        max_width_pt: f32,
        line_height_mm: f32,
    ) {
        self.ensure_space(line_height_mm);
        let max_size = line
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Box(word, _) => word.iter().map(|glyph| glyph.size).reduce(f32::max),
                _ => None,
            })
            .fold(0.0_f32, f32::max)
            .max(1.0);
        let baseline = self.y - max_size * 0.8 / PT_PER_MM;
        self.draw_shaped_line_at(line, self.margin + indent, baseline, align, max_width_pt);
        self.y -= line_height_mm;
        self.has_content = true;
    }

    fn draw_shaped_line_at(
        &mut self,
        line: &ShapedLine,
        x_mm: f32,
        baseline: f32,
        align: Align,
        max_width_pt: f32,
    ) {
        let line_width_pt = line.width;
        let offset_mm = match align {
            Align::Center => ((max_width_pt - line_width_pt) / 2.0 / PT_PER_MM).max(0.0),
            Align::Right => ((max_width_pt - line_width_pt) / PT_PER_MM).max(0.0),
            Align::Left | Align::Justify => 0.0,
        };
        let mut x = x_mm + offset_mm;
        let mut run: Option<ShapedRun> = None;

        for item in &line.items {
            match item {
                Item::Box(word, _) => {
                    for glyph in word {
                        if run.as_ref().is_none_or(|run| {
                            run.face != glyph.face
                                || (run.size - glyph.size).abs() > 0.01
                                || run.color != glyph.color
                        }) {
                            self.flush_run(&mut run, baseline);
                            run = Some(ShapedRun {
                                face: glyph.face,
                                size: glyph.size,
                                color: glyph.color.clone(),
                                start_mm: x,
                                items: Vec::new(),
                            });
                        }
                        let run = run.as_mut().unwrap();
                        let scale = 1000.0 / glyph.size;
                        let font_advance = self
                            .fonts
                            .face(glyph.face)
                            .width_gid(glyph.gid, glyph.size)
                            .unwrap_or(glyph.advance);
                        let x_offset = glyph.x_offset * scale;
                        let correction = (glyph.advance - font_advance) * scale - x_offset;
                        if x_offset.abs() > 0.01 {
                            run.items.push(TextItem::Offset(x_offset));
                        }
                        run.items.push(TextItem::GlyphIds(vec![Codepoint::with_cid(
                            glyph.gid,
                            0.0,
                            glyph.cid.clone(),
                        )]));
                        if correction.abs() > 0.01 {
                            run.items.push(TextItem::Offset(correction));
                        }
                        x += glyph.advance / PT_PER_MM;
                    }
                }
                Item::Glue { width, .. } => {
                    if let Some(run) = run.as_mut() {
                        let font_advance = self
                            .fonts
                            .face(run.face)
                            .width(' ', run.size)
                            .unwrap_or(*width);
                        run.items.push(TextItem::Text(" ".to_string()));
                        let correction = (width - font_advance) * 1000.0 / run.size;
                        if correction.abs() > 0.01 {
                            run.items.push(TextItem::Offset(correction));
                        }
                    }
                    x += width / PT_PER_MM;
                }
                Item::Penalty { width, .. } => {
                    x += width / PT_PER_MM;
                }
            }
        }
        self.flush_run(&mut run, baseline);
    }

    fn flush_run(&mut self, run: &mut Option<ShapedRun>, baseline: f32) {
        let Some(run) = run.take() else {
            return;
        };
        if run.items.is_empty() {
            return;
        }
        self.ops.push(Op::SetFont {
            font: PdfFontHandle::External(self.fonts.face(run.face).id.clone()),
            size: Pt(run.size),
        });
        self.ops.push(Op::SetFillColor {
            col: pdf_color(&run.color),
        });
        self.ops.push(Op::SetTextMatrix {
            matrix: TextMatrix::Translate(Pt(run.start_mm * PT_PER_MM), Pt(baseline * PT_PER_MM)),
        });
        self.ops.push(Op::ShowText { items: run.items });
    }
    fn render_code_block(&mut self, text: &str, style: &Style, indent: f32) {
        let text = text.trim_end_matches('\n');
        let size = style.size_or(13.0);
        let line_height = size * style.leading_or(1.55) / PT_PER_MM;
        let max_width_pt = self.content_width(indent) * PT_PER_MM;
        let color = style.color_or("#24292f");
        for raw_line in text.split('\n') {
            let chars: Vec<StyledChar> = raw_line
                .chars()
                .map(|ch| StyledChar {
                    ch,
                    size,
                    color: color.to_string(),
                    face: FaceKind::Mono,
                })
                .collect();
            let mut line = Vec::new();
            let mut width = 0.0;
            for ch in chars {
                let char_width = self.char_width(&ch);
                if width + char_width > max_width_pt && !line.is_empty() {
                    self.draw_line(&line, indent, Align::Left, max_width_pt, line_height);
                    line.clear();
                    width = 0.0;
                }
                width += char_width;
                line.push(ch);
            }
            self.draw_line(&line, indent, Align::Left, max_width_pt, line_height);
        }
        self.space(style.margin_after_or(12.0));
    }

    fn layout(&self, chars: &[StyledChar], max_width_pt: f32) -> Vec<Vec<StyledChar>> {
        let mut lines = Vec::new();
        let mut line = Vec::new();
        let mut line_width = 0.0;
        let mut atom = Vec::new();
        let mut atom_width = 0.0;

        let flush_atom = |line: &mut Vec<StyledChar>,
                          line_width: &mut f32,
                          atom: &mut Vec<StyledChar>,
                          atom_width: &mut f32,
                          lines: &mut Vec<Vec<StyledChar>>,
                          max_width_pt: f32| {
            if atom.is_empty() {
                return;
            }
            if *line_width + *atom_width > max_width_pt && !line.is_empty() {
                lines.push(std::mem::take(line));
                *line_width = 0.0;
            }
            if *atom_width > max_width_pt {
                for ch in atom.drain(..) {
                    let width = self.char_width(&ch);
                    if *line_width + width > max_width_pt && !line.is_empty() {
                        lines.push(std::mem::take(line));
                        *line_width = 0.0;
                    }
                    *line_width += width;
                    line.push(ch);
                }
            } else {
                line.append(atom);
                *line_width += *atom_width;
            }
            *atom_width = 0.0;
        };

        for ch in chars {
            if ch.ch == '\n' {
                flush_atom(
                    &mut line,
                    &mut line_width,
                    &mut atom,
                    &mut atom_width,
                    &mut lines,
                    max_width_pt,
                );
                lines.push(std::mem::take(&mut line));
                line_width = 0.0;
                continue;
            }

            if ch.ch.is_whitespace() {
                flush_atom(
                    &mut line,
                    &mut line_width,
                    &mut atom,
                    &mut atom_width,
                    &mut lines,
                    max_width_pt,
                );
                if line.is_empty() {
                    continue;
                }
                let space = StyledChar {
                    ch: ' ',
                    ..ch.clone()
                };
                let width = self.char_width(&space);
                if line_width + width > max_width_pt {
                    lines.push(std::mem::take(&mut line));
                    line_width = 0.0;
                } else {
                    line_width += width;
                    line.push(space);
                }
                continue;
            }

            if is_cjk(ch.ch) {
                flush_atom(
                    &mut line,
                    &mut line_width,
                    &mut atom,
                    &mut atom_width,
                    &mut lines,
                    max_width_pt,
                );
                let width = self.char_width(ch);
                if line_width + width > max_width_pt && !line.is_empty() {
                    lines.push(std::mem::take(&mut line));
                    line_width = 0.0;
                }
                line_width += width;
                line.push(ch.clone());
                continue;
            }

            atom_width += self.char_width(ch);
            atom.push(ch.clone());
        }

        flush_atom(
            &mut line,
            &mut line_width,
            &mut atom,
            &mut atom_width,
            &mut lines,
            max_width_pt,
        );
        if !line.is_empty() {
            lines.push(line);
        }
        lines
    }

    fn draw_line(
        &mut self,
        line: &[StyledChar],
        indent: f32,
        align: Align,
        max_width_pt: f32,
        line_height_mm: f32,
    ) {
        self.ensure_space(line_height_mm);
        let max_size = line
            .iter()
            .map(|ch| ch.size)
            .fold(0.0_f32, f32::max)
            .max(1.0);
        let baseline = self.y - max_size * 0.8 / PT_PER_MM;
        self.draw_line_at(line, self.margin + indent, baseline, align, max_width_pt);
        self.y -= line_height_mm;
        self.has_content = true;
    }

    fn draw_line_at(
        &mut self,
        line: &[StyledChar],
        x_mm: f32,
        baseline: f32,
        align: Align,
        max_width_pt: f32,
    ) {
        let line_width_pt: f32 = line.iter().map(|ch| self.char_width(ch)).sum();
        let offset_mm = match align {
            Align::Center => ((max_width_pt - line_width_pt) / 2.0 / PT_PER_MM).max(0.0),
            Align::Right => ((max_width_pt - line_width_pt) / PT_PER_MM).max(0.0),
            Align::Left | Align::Justify => 0.0,
        };
        let mut x = x_mm + offset_mm;

        let mut index = 0;
        while index < line.len() {
            let kind = self.fonts.face_for_char(line[index].ch, line[index].face);
            let color = line[index].color.clone();
            let size = line[index].size;
            let start = index;
            let mut width = 0.0;
            while index < line.len() {
                let current = &line[index];
                if self.fonts.face_for_char(current.ch, current.face) != kind
                    || current.color != color
                    || (current.size - size).abs() > 0.01
                {
                    break;
                }
                width += self.char_width(current);
                index += 1;
            }
            let text: String = line[start..index].iter().map(|ch| ch.ch).collect();
            let face = self.fonts.face(kind);
            self.ops.push(Op::SetFont {
                font: PdfFontHandle::External(face.id.clone()),
                size: Pt(size),
            });
            self.ops.push(Op::SetFillColor {
                col: pdf_color(&color),
            });
            self.ops.push(Op::SetTextMatrix {
                matrix: TextMatrix::Translate(Pt(x * PT_PER_MM), Pt(baseline * PT_PER_MM)),
            });
            self.ops.push(Op::ShowText {
                items: vec![TextItem::Text(text)],
            });
            x += width / PT_PER_MM;
        }
    }
    fn draw_rule(&mut self, indent: f32) {
        let x1 = self.margin + indent;
        let x2 = self.width - self.margin;
        let y = self.y;
        self.ops.push(Op::EndTextSection);
        self.ops.push(Op::SetOutlineColor {
            col: pdf_color("#d8dee4"),
        });
        self.ops.push(Op::SetOutlineThickness { pt: Pt(0.7) });
        self.ops.push(Op::DrawLine {
            line: Line {
                points: vec![
                    LinePoint {
                        p: Point::new(Mm(x1), Mm(y)),
                        bezier: false,
                    },
                    LinePoint {
                        p: Point::new(Mm(x2), Mm(y)),
                        bezier: false,
                    },
                ],
                is_closed: false,
            },
        });
        self.ops.push(Op::StartTextSection);
    }

    fn char_width(&self, ch: &StyledChar) -> f32 {
        let kind = self.fonts.face_for_char(ch.ch, ch.face);
        let face = self.fonts.face(kind);
        face.width(ch.ch, ch.size).unwrap_or(if is_cjk(ch.ch) {
            ch.size
        } else {
            ch.size * 0.5
        })
    }
}

fn image_path(src: &str) -> PathBuf {
    let path = PathBuf::from(src);
    if path.is_absolute() {
        path
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    }
}

fn image_dimensions(
    image: &Image,
    pixel_width: usize,
    pixel_height: usize,
    content_width: f32,
    max_height: f32,
) -> (f32, f32) {
    let aspect = if pixel_height == 0 {
        1.0
    } else {
        pixel_width as f32 / pixel_height as f32
    };
    let mut width = match image.width {
        Some(ImageSize::Percent(value)) => content_width * value / 100.0,
        Some(ImageSize::Points(value)) => value / PT_PER_MM,
        None => match image.height {
            Some(_) => 0.0,
            None => content_width,
        },
    };
    let mut height = match image.height {
        Some(ImageSize::Percent(value)) => max_height * value / 100.0,
        Some(ImageSize::Points(value)) => value / PT_PER_MM,
        None => {
            if width > 0.0 {
                width / aspect.max(0.01)
            } else {
                0.0
            }
        }
    };
    if image.width.is_none() && image.height.is_some() {
        width = height * aspect;
    }
    if width <= 0.0 {
        width = content_width;
        height = width / aspect.max(0.01);
    }
    let width_scale = if width > content_width {
        content_width / width
    } else {
        1.0
    };
    let height_scale = if height > max_height {
        max_height / height
    } else {
        1.0
    };
    let scale = width_scale.min(height_scale);
    (width * scale, height * scale)
}

fn flush_word(items: &mut Vec<Item<Word>>, word: &mut Word, width: &mut f32, line_width_pt: f32) {
    if word.is_empty() {
        *width = 0.0;
        return;
    }
    let word_width = *width;
    let glyphs = std::mem::take(word);
    if word_width > line_width_pt + 0.01 && glyphs.len() > 1 {
        let count = glyphs.len();
        for (index, glyph) in glyphs.into_iter().enumerate() {
            let advance = glyph.advance;
            items.push(Item::Box(vec![glyph], advance));
            if index + 1 < count {
                items.push(Item::Penalty {
                    width: 0.0,
                    penalty: 1_000.0,
                });
            }
        }
    } else {
        items.push(Item::Box(glyphs, word_width));
    }
    *width = 0.0;
}

fn collect_inlines(inlines: &[Inline], style: &Style, theme: &Theme) -> Vec<StyledChar> {
    let mut chars = Vec::new();
    collect_inline_recursive(inlines, style, theme, &mut chars, false);
    chars
}

fn collect_inline_recursive(
    inlines: &[Inline],
    style: &Style,
    theme: &Theme,
    out: &mut Vec<StyledChar>,
    bold: bool,
) {
    let size = style.size_or(15.0);
    let color = style.color_or("#24292f");
    let face = if style.font.as_deref() == Some("Cascadia Mono") {
        FaceKind::Mono
    } else if bold || style.weight_or(400) >= 600 {
        FaceKind::Bold
    } else {
        FaceKind::Regular
    };

    for inline in inlines {
        match inline {
            Inline::Text(text) => {
                for ch in text.chars() {
                    out.push(StyledChar {
                        ch,
                        size,
                        color: color.to_string(),
                        face,
                    });
                }
            }
            Inline::Strong(children) => {
                collect_inline_recursive(children, style, theme, out, true);
            }
            Inline::Emphasis(children) => {
                collect_inline_recursive(children, style, theme, out, bold);
            }
            Inline::Code(text) => {
                let code_style = theme.style("code");
                let code_size = code_style.size_or(13.0);
                let code_color = code_style.color_or("#b31d28");
                for ch in text.chars() {
                    out.push(StyledChar {
                        ch,
                        size: code_size,
                        color: code_color.to_string(),
                        face: FaceKind::Mono,
                    });
                }
            }
            Inline::Link { children, .. } => {
                let link_style = Style {
                    color: Some("#0969da".to_string()),
                    ..style.clone()
                };
                collect_inline_recursive(children, &link_style, theme, out, bold);
            }
            Inline::Image(image) => {
                for ch in format!("[图片: {}]", image.src).chars() {
                    out.push(StyledChar {
                        ch,
                        size,
                        color: color.to_string(),
                        face,
                    });
                }
            }
            Inline::Reference { index } => {
                for ch in format!("[{index}]").chars() {
                    out.push(StyledChar {
                        ch,
                        size,
                        color: "#0969da".to_string(),
                        face,
                    });
                }
            }
            Inline::Break => out.push(StyledChar {
                ch: '\n',
                size,
                color: color.to_string(),
                face,
            }),
        }
    }
}

fn pdf_color(hex: &str) -> Color {
    let (r, g, b) = parse_color(hex);
    Color::Rgb(Rgb {
        r,
        g,
        b,
        icc_profile: None,
    })
}

fn parse_color(hex: &str) -> (f32, f32, f32) {
    let hex = hex.trim().trim_start_matches('#');
    let expanded = if hex.len() == 3 {
        hex.chars().flat_map(|ch| [ch, ch]).collect::<String>()
    } else {
        hex.to_string()
    };
    if expanded.len() >= 6 {
        let r = u8::from_str_radix(&expanded[0..2], 16).unwrap_or(36) as f32 / 255.0;
        let g = u8::from_str_radix(&expanded[2..4], 16).unwrap_or(41) as f32 / 255.0;
        let b = u8::from_str_radix(&expanded[4..6], 16).unwrap_or(47) as f32 / 255.0;
        (r, g, b)
    } else {
        (0.14, 0.16, 0.18)
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
