use std::fmt::Write;
use std::path::Path;

use docscript::{
    Align, Document, Image, ImageSize, Inline, ListItem, Node, Style, TableCell, TableRow, Theme,
};

pub fn render_html(document: &Document) -> String {
    let theme = &document.theme;
    let mut out = String::new();
    out.push_str("<!doctype html>\n<html lang=\"zh-CN\">\n<head>\n<meta charset=\"utf-8\">\n");
    out.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n");
    let _ = writeln!(out, "<title>{}</title>", escape_html(&document.title));
    out.push_str("<style>\n");
    write_css(&mut out, theme);
    out.push_str("</style>\n</head>\n<body>\n");
    out.push_str("<main class=\"page\">\n");

    for block in &document.blocks {
        write_node(&mut out, block, theme);
    }

    out.push_str("</main>\n</body>\n</html>\n");
    out
}

pub fn write_node(out: &mut String, node: &Node, theme: &Theme) {
    write_node_with(out, node, theme, None);
}

fn write_node_with(out: &mut String, node: &Node, theme: &Theme, inherited: Option<&Style>) {
    match node {
        Node::Paragraph { inlines, style } => {
            let mut resolved = theme.style("text");
            if let Some(inherited) = inherited {
                resolved = resolved.merged(&inherited.text_inheritance());
            }
            if let Some(style) = style {
                resolved = resolved.merged(style);
            }
            let _ = write!(
                out,
                "<p class=\"paragraph\" style=\"{}\">",
                style_css(&resolved)
            );
            write_inlines(out, inlines);
            out.push_str("</p>\n");
        }
        Node::Heading {
            level,
            inlines,
            style,
        } => {
            let level = (*level).clamp(1, 6);
            let mut resolved = theme.heading_style(level);
            if let Some(inherited) = inherited {
                resolved = resolved.merged(&inherited.font_color_inheritance());
            }
            if let Some(style) = style {
                resolved = resolved.merged(style);
            }
            let _ = write!(
                out,
                "<h{level} class=\"heading h{level}\" style=\"{}\">",
                style_css(&resolved)
            );
            write_inlines(out, inlines);
            let _ = writeln!(out, "</h{level}>");
        }
        Node::Quote { blocks, style } => {
            let mut resolved = theme.style("quote");
            if let Some(style) = style {
                resolved = resolved.merged(style);
            }
            let child_style = resolved.text_inheritance();
            let _ = writeln!(
                out,
                "<blockquote class=\"quote\" style=\"{}\">",
                style_css(&resolved)
            );
            for block in blocks {
                write_node_with(out, block, theme, Some(&child_style));
            }
            out.push_str("</blockquote>\n");
        }
        Node::List {
            ordered,
            items,
            style,
        } => {
            let mut resolved = theme.style("list");
            if let Some(style) = style {
                resolved = resolved.merged(style);
            }
            let child_style = resolved.text_inheritance();
            let tag = if *ordered { "ol" } else { "ul" };
            let _ = writeln!(
                out,
                "<{tag} class=\"list\" style=\"{}\">",
                style_css(&resolved)
            );
            for item in items {
                out.push_str("<li>");
                write_item(out, item, theme, Some(&child_style));
                out.push_str("</li>\n");
            }
            let _ = writeln!(out, "</{tag}>");
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
            let item = ListItem::new(blocks.clone()).with_style(inherited);
            write_item(out, &item, theme, None);
        }
        Node::CodeBlock { lang, text, style } => {
            let mut resolved = theme.style("code_block");
            if let Some(style) = style {
                resolved = resolved.merged(style);
            }
            let _ = write!(
                out,
                "<pre class=\"code-block\" style=\"{}\"><code",
                style_css(&resolved)
            );
            if let Some(lang) = lang {
                let _ = write!(out, " class=\"language-{}\"", escape_html(lang));
            }
            out.push('>');
            out.push_str(&escape_html(text));
            out.push_str("</code></pre>\n");
        }
        Node::Divider { style } => {
            let (before, after) = style
                .as_ref()
                .map(|style| (style.margin_before_or(18.0), style.margin_after_or(18.0)))
                .unwrap_or((18.0, 18.0));
            let _ = writeln!(
                out,
                "<hr class=\"divider\" style=\"margin: {}px 0 {}px;\">",
                before, after
            );
        }
        Node::Figure {
            image,
            caption,
            style,
        } => {
            let mut resolved = theme.style("figure");
            if let Some(style) = style {
                resolved = resolved.merged(style);
            }
            let _ = writeln!(
                out,
                "<figure class=\"figure\" style=\"{}\">",
                style_css(&resolved)
            );
            let caption_text = caption.iter().map(Inline::plain_text).collect::<String>();
            write_image(
                out,
                image,
                (!caption_text.trim().is_empty()).then_some(caption_text.trim()),
            );
            if !caption.is_empty() {
                let mut caption_style = theme.style("caption");
                if let Some(align) = resolved.align {
                    caption_style.align = Some(align);
                }
                let _ = writeln!(
                    out,
                    "<figcaption class=\"caption\" style=\"{}\">",
                    style_css(&caption_style)
                );
                write_inlines(out, caption);
                out.push_str("</figcaption>\n");
            }
            out.push_str("</figure>\n");
        }
        Node::Table { rows, style } => {
            let mut resolved = theme.style("table");
            if let Some(style) = style {
                resolved = resolved.merged(style);
            }
            let _ = writeln!(
                out,
                "<table class=\"table\" style=\"{}\"><tbody>",
                style_css(&resolved)
            );
            let columns = rows.iter().map(|row| row.cells.len()).max().unwrap_or(0);
            for row in rows {
                write_table_row(out, row, theme, Some(&resolved), columns);
            }
            out.push_str("</tbody></table>\n");
        }
        Node::TableRow { cells, style } => {
            let resolved = theme.style("table");
            let _ = writeln!(
                out,
                "<table class=\"table\" style=\"{}\"><tbody>",
                style_css(&resolved)
            );
            let row = TableRow::new(cells.clone()).with_style(style.clone());
            write_table_row(out, &row, theme, Some(&resolved), cells.len());
            out.push_str("</tbody></table>\n");
        }
        Node::TableCell { blocks, style } => {
            let resolved = theme.style("table");
            let _ = writeln!(
                out,
                "<table class=\"table\" style=\"{}\"><tbody><tr>",
                style_css(&resolved)
            );
            let cell = TableCell::new(blocks.clone()).with_style(style.clone());
            let row_style = theme.style("row");
            write_table_cell(out, &cell, theme, &row_style);
            out.push_str("</tr></tbody></table>\n");
        }
        Node::Bibliography { entries, style } => {
            let mut resolved = theme.style("bibliography");
            if let Some(style) = style {
                resolved = resolved.merged(style);
            }
            let title_style = theme.heading_style(2);
            let mut entry_style = theme.style("text");
            entry_style.size = Some(13.0);
            entry_style.leading = Some(1.5);
            entry_style.color = Some("#374151".to_string());
            let _ = writeln!(
                out,
                "<section class=\"bibliography\" style=\"{}\">",
                style_css(&resolved)
            );
            let _ = writeln!(
                out,
                "<h2 class=\"heading h2 bibliography-title\" style=\"{}\">参考书目</h2>",
                style_css(&title_style)
            );
            out.push_str("<ol class=\"bibliography-list\">\n");
            for (index, entry) in entries.iter().enumerate() {
                let _ = writeln!(
                    out,
                    "<li id=\"reference-{}\" class=\"bibliography-entry\" style=\"{}\">",
                    index + 1,
                    style_css(&entry_style)
                );
                write_inlines(out, &entry.inlines);
                out.push_str("</li>\n");
            }
            out.push_str("</ol>\n</section>\n");
        }
        Node::Custom {
            name,
            blocks,
            style,
        } => {
            let mut resolved = theme.style(name);
            if let Some(style) = style {
                resolved = resolved.merged(style);
            }
            let child_style = resolved.text_inheritance();
            let _ = writeln!(
                out,
                "<section class=\"custom custom-{name}\" data-element=\"{name}\" style=\"{}\">",
                style_css(&resolved)
            );
            for block in blocks {
                write_node_with(out, block, theme, Some(&child_style));
            }
            out.push_str("</section>\n");
        }
    }
}

fn write_table_row(
    out: &mut String,
    row: &TableRow,
    theme: &Theme,
    inherited: Option<&Style>,
    columns: usize,
) {
    let mut resolved = theme.style("row");
    if let Some(inherited) = inherited {
        resolved = resolved.merged(&inherited.text_inheritance());
    }
    if let Some(style) = &row.style {
        resolved = resolved.merged(style);
    }
    let _ = writeln!(
        out,
        "<tr class=\"table-row\" style=\"{}\">",
        style_css(&resolved)
    );
    for index in 0..columns.max(row.cells.len()) {
        match row.cells.get(index) {
            Some(cell) => write_table_cell(out, cell, theme, &resolved),
            None => write_table_cell(out, &TableCell::new(Vec::new()), theme, &resolved),
        }
    }
    out.push_str("</tr>\n");
}

fn write_table_cell(out: &mut String, cell: &TableCell, theme: &Theme, row_style: &Style) {
    let mut resolved = theme.style("cell").merged(&row_style.text_inheritance());
    if let Some(style) = &cell.style {
        resolved = resolved.merged(style);
    }
    let child_style = resolved.text_inheritance();
    let _ = writeln!(
        out,
        "<td class=\"table-cell\" style=\"{}\">",
        style_css(&resolved)
    );
    for block in &cell.blocks {
        write_node_with(out, block, theme, Some(&child_style));
    }
    out.push_str("</td>");
}

fn write_item(out: &mut String, item: &ListItem, theme: &Theme, inherited: Option<&Style>) {
    let mut inherited = inherited.cloned();
    if let Some(style) = &item.style {
        inherited = Some(
            inherited
                .map(|inherited| inherited.merged(style))
                .unwrap_or_else(|| style.clone()),
        );
    }
    for block in &item.blocks {
        write_node_with(out, block, theme, inherited.as_ref());
    }
}

pub fn write_inlines(out: &mut String, inlines: &[Inline]) {
    for inline in inlines {
        match inline {
            Inline::Text(text) => out.push_str(&escape_html(text)),
            Inline::Strong(children) => {
                out.push_str("<strong>");
                write_inlines(out, children);
                out.push_str("</strong>");
            }
            Inline::Emphasis(children) => {
                out.push_str("<em>");
                write_inlines(out, children);
                out.push_str("</em>");
            }
            Inline::Code(text) => {
                out.push_str("<code class=\"inline-code\">");
                out.push_str(&escape_html(text));
                out.push_str("</code>");
            }
            Inline::Link { target, children } => {
                let _ = write!(out, "<a class=\"link\" href=\"{}\">", escape_html(target));
                write_inlines(out, children);
                out.push_str("</a>");
            }
            Inline::Image(image) => write_image(out, image, None),
            Inline::Reference { index } => {
                let _ = write!(
                    out,
                    "<sup class=\"reference\"><a href=\"#reference-{index}\">[{index}]</a></sup>"
                );
            }
            Inline::Break => out.push_str("<br>"),
        }
    }
}

fn write_image(out: &mut String, image: &Image, alt: Option<&str>) {
    let path = Path::new(&image.src);
    let src = std::fs::read(path)
        .ok()
        .map(|bytes| format!("data:{};base64,{}", image_mime(path), base64_encode(&bytes)))
        .unwrap_or_else(|| image.src.clone());
    let alt = alt.map(str::to_string).unwrap_or_else(|| {
        path.file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "image".to_string())
    });
    let mut css = String::from("max-width: 100%; height: auto; vertical-align: middle; ");
    if let Some(width) = image.width {
        let _ = write!(css, "width: {}; ", css_size(width));
    } else if image.height.is_none() {
        css.push_str("width: 100%; ");
    }
    if let Some(height) = image.height {
        let _ = write!(css, "height: {}; ", css_size(height));
    }
    let _ = write!(
        out,
        "<img class=\"image\" src=\"{}\" alt=\"{}\" style=\"{}\">",
        escape_html(&src),
        escape_html(&alt),
        css
    );
}

fn css_size(size: ImageSize) -> String {
    match size {
        ImageSize::Percent(value) => format!("{value}%"),
        ImageSize::Points(value) => format!("{value}pt"),
    }
}

fn image_mime(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        _ => "application/octet-stream",
    }
}

fn base64_encode(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
        let b2 = chunk.get(2).copied().unwrap_or(0) as u32;
        let value = (b0 << 16) | (b1 << 8) | b2;
        out.push(TABLE[((value >> 18) & 0x3f) as usize] as char);
        out.push(TABLE[((value >> 12) & 0x3f) as usize] as char);
        out.push(if chunk.len() > 1 {
            TABLE[((value >> 6) & 0x3f) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[(value & 0x3f) as usize] as char
        } else {
            '='
        });
    }
    out
}

fn write_css(out: &mut String, theme: &Theme) {
    let page = &theme.page;
    let base = theme.style("text");
    let _ = writeln!(
        out,
        ":root {{ color-scheme: light; --page-width: {}mm; --page-margin: {}mm; }}",
        page.width, page.margin
    );
    out.push_str("* { box-sizing: border-box; }\n");
    out.push_str("html, body { margin: 0; padding: 0; background: #eef1f5; }\n");
    out.push_str("body { display: flex; justify-content: center; padding: 28px 16px; }\n");
    let _ = writeln!(
        out,
        ".page {{ width: {}mm; min-height: {}mm; padding: {}mm; margin: 0 auto; background: white; box-shadow: 0 10px 40px rgba(15, 23, 42, .14); }}",
        page.width, page.height, page.margin
    );
    let _ = writeln!(out, "body {{ {} }}", style_css(&base));
    out.push_str("body { margin: 0; }\n");
    let _ = writeln!(out, ".paragraph {{ {} }}", style_css(&base));
    let _ = writeln!(
        out,
        ".quote {{ {} border-left: 3px solid #d0d7de; padding-left: 16px; }}",
        style_css(&theme.style("quote"))
    );
    let _ = writeln!(
        out,
        ".list {{ {} padding-left: 1.6em; }}",
        style_css(&theme.style("list"))
    );
    for level in 1..=6 {
        let _ = writeln!(
            out,
            ".h{level} {{ {} }}",
            style_css(&theme.heading_style(level))
        );
    }
    let code = theme.style("code");
    let _ = writeln!(
        out,
        ".inline-code {{ font-family: {}; font-size: {}px; color: {}; background: {}; padding: .12em .34em; border-radius: 4px; }}",
        css_font(&code),
        code.size_or(13.0),
        code.color_or("#b31d28"),
        code.background.as_deref().unwrap_or("#f3f4f6")
    );
    let code_block = theme.style("code_block");
    let _ = writeln!(
        out,
        ".code-block {{ {} padding: 14px 16px; border-radius: 8px; overflow-x: auto; white-space: pre-wrap; word-break: break-word; }}",
        style_css(&code_block)
    );
    let _ = writeln!(
        out,
        ".link {{ color: #0969da; text-decoration: none; border-bottom: 1px solid rgba(9,105,218,.35); }}"
    );
    out.push_str(".divider { border: 0; border-top: 1px solid #d8dee4; margin: 24px 0; }\n");
    out.push_str(".custom { display: block; border-radius: 6px; }\n");
    out.push_str(".image { display: inline-block; }\n");
    out.push_str(".figure { display: block; }\n");
    out.push_str(".table { width: 100%; table-layout: fixed; border-collapse: collapse; }\n");
    out.push_str(".table-cell { padding: 8px; border: 1px solid #d8dee4; vertical-align: top; }\n");
    out.push_str(".reference a { color: #0969da; text-decoration: none; }\n");
    out.push_str(".bibliography { border-top: 1px solid #d8dee4; padding-top: 12px; }\n");
    out.push_str(".bibliography-list { padding-left: 1.4em; }\n");
    out.push_str("strong { font-weight: 700; }\n");
    out.push_str("@media print { body { background: white; padding: 0; } .page { box-shadow: none; margin: 0; width: auto; min-height: 0; padding: 0; } }\n");
    let _ = writeln!(
        out,
        "@page {{ size: {}mm {}mm; margin: {}mm; }}",
        page.width, page.height, page.margin
    );
    out.push_str("h1, h2, h3, h4, h5, h6 { break-after: avoid; }\n");
    out.push_str("pre, blockquote { break-inside: avoid; }\n");
}

fn style_css(style: &Style) -> String {
    let mut css = String::new();
    let _ = write!(css, "font-family: {}; ", css_font(style));
    let _ = write!(css, "font-size: {}px; ", style.size_or(15.0));
    let _ = write!(css, "font-weight: {}; ", style.weight_or(400));
    if style.italic.unwrap_or(false) {
        css.push_str("font-style: italic; ");
    }
    let _ = write!(css, "color: {}; ", style.color_or("#24292f"));
    if let Some(background) = &style.background {
        let _ = write!(css, "background: {background}; ");
    }
    let _ = write!(css, "line-height: {}; ", style.leading_or(1.75));
    let _ = write!(
        css,
        "text-align: {}; ",
        match style.align_or(Align::Left) {
            Align::Left => "left",
            Align::Center => "center",
            Align::Right => "right",
            Align::Justify => "justify",
        }
    );
    if let Some(indent) = style.indent {
        let _ = write!(css, "text-indent: {}px; ", indent);
    }
    let _ = write!(
        css,
        "margin: {}px 0 {}px; ",
        style.margin_before_or(0.0),
        style.margin_after_or(10.0)
    );
    if let Some(tracking) = style.tracking {
        let _ = write!(css, "letter-spacing: {}px; ", tracking);
    }
    css
}

fn css_font(style: &Style) -> String {
    let font = style.font.as_deref().unwrap_or("Microsoft YaHei");
    format!(
        "\"{}\", \"Microsoft YaHei\", \"Noto Sans CJK SC\", sans-serif",
        font.replace('"', "")
    )
}

pub fn escape_html(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            other => out.push(other),
        }
    }
    out
}
