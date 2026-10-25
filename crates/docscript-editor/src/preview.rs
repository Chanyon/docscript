use std::path::PathBuf;

use docscript::{
    Align, Document, Image, ImageSize, Inline, ListItem, Node, Style, TableCell, TableRow, Theme,
};
use gpui::{
    AnyElement, FontStyle, FontWeight, Hsla, ObjectFit, StyledText, TextAlign, TextRun,
    UnderlineStyle, div, font, img, prelude::*, px, relative, rgb,
};

pub fn document_view(document: &Document) -> AnyElement {
    let theme = &document.theme;
    div()
        .flex()
        .flex_col()
        .w_full()
        .max_w(px(760.0))
        .min_h(px(900.0))
        .p(px(52.0))
        .bg(gpui::white())
        .shadow_lg()
        .children(document.blocks.iter().map(|node| node_view(node, theme)))
        .into_any_element()
}

fn node_view(node: &Node, theme: &Theme) -> AnyElement {
    node_view_with(node, theme, None)
}

fn node_view_with(node: &Node, theme: &Theme, inherited: Option<&Style>) -> AnyElement {
    match node {
        Node::Paragraph { inlines, style } => {
            let mut resolved = theme.style("text");
            if let Some(inherited) = inherited {
                resolved = resolved.merged(&inherited.text_inheritance());
            }
            if let Some(style) = style {
                resolved = resolved.merged(style);
            }
            paragraph_view(inlines, &resolved, theme, false)
        }
        Node::Heading {
            level,
            inlines,
            style,
        } => {
            let mut resolved = theme.heading_style(*level);
            if let Some(inherited) = inherited {
                resolved = resolved.merged(&inherited.font_color_inheritance());
            }
            if let Some(style) = style {
                resolved = resolved.merged(style);
            }
            paragraph_view(inlines, &resolved, theme, true)
        }
        Node::Quote { blocks, style } => {
            let mut resolved = theme.style("quote");
            if let Some(style) = style {
                resolved = resolved.merged(style);
            }
            let child_style = resolved.text_inheritance();
            let size = resolved.size_or(15.0);
            div()
                .flex()
                .flex_col()
                .border_l_4()
                .border_color(rgb(0xd0d7de))
                .pl(px(resolved.indent_or(18.0)))
                .mt(px(resolved.margin_before_or(12.0)))
                .mb(px(resolved.margin_after_or(12.0)))
                .text_size(px(size))
                .text_color(rgb(parse_hex(resolved.color_or("#57606a"))))
                .children(
                    blocks
                        .iter()
                        .map(|node| node_view_with(node, theme, Some(&child_style))),
                )
                .into_any_element()
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
            let muted = rgb(0x6b7280);
            div()
                .flex()
                .flex_col()
                .gap_2()
                .mt(px(resolved.margin_before_or(8.0)))
                .mb(px(resolved.margin_after_or(8.0)))
                .children(items.iter().enumerate().map(|(index, item)| {
                    let marker = if *ordered {
                        format!("{}.", index + 1)
                    } else {
                        "•".to_string()
                    };
                    div()
                        .flex()
                        .flex_row()
                        .items_start()
                        .gap_2()
                        .child(
                            div()
                                .w(px(24.0))
                                .flex_shrink_0()
                                .pt(px(1.0))
                                .text_color(muted)
                                .child(marker),
                        )
                        .child(div().flex().flex_col().flex_1().child(list_item_view(
                            item,
                            theme,
                            Some(&child_style),
                        )))
                }))
                .into_any_element()
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
            div()
                .flex()
                .flex_col()
                .children(
                    blocks
                        .iter()
                        .map(|node| node_view_with(node, theme, inherited.as_ref())),
                )
                .into_any_element()
        }
        Node::CodeBlock { text, style, .. } => {
            let mut resolved = theme.style("code_block");
            if let Some(style) = style {
                resolved = resolved.merged(style);
            }
            let size = resolved.size_or(13.0);
            let line_height = size * resolved.leading_or(1.55);
            div()
                .flex()
                .flex_col()
                .p_4()
                .rounded_md()
                .overflow_hidden()
                .bg(rgb(parse_hex(
                    resolved.background.as_deref().unwrap_or("#f6f8fa"),
                )))
                .mt(px(resolved.margin_before_or(12.0)))
                .mb(px(resolved.margin_after_or(12.0)))
                .font_family(
                    resolved
                        .font
                        .clone()
                        .unwrap_or_else(|| "Cascadia Mono".to_string()),
                )
                .text_size(px(size))
                .line_height(px(line_height))
                .text_color(rgb(parse_hex(resolved.color_or("#24292f"))))
                .children(text.lines().map(|line| {
                    if line.trim().is_empty() {
                        div().h(px(line_height)).into_any_element()
                    } else {
                        div()
                            .whitespace_nowrap()
                            .child(line.to_string())
                            .into_any_element()
                    }
                }))
                .into_any_element()
        }
        Node::Divider { style } => {
            let (before, after) = style
                .as_ref()
                .map(|style| (style.margin_before_or(18.0), style.margin_after_or(18.0)))
                .unwrap_or((18.0, 18.0));
            div()
                .h(px(1.0))
                .w_full()
                .mt(px(before))
                .mb(px(after))
                .bg(rgb(0xd8dee4))
                .into_any_element()
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
            let mut caption_style = theme.style("caption");
            caption_style.margin_before = Some(caption_style.margin_before.unwrap_or(6.0));
            if let Some(align) = resolved.align {
                caption_style.align = Some(align);
            }
            let mut container = div()
                .flex()
                .flex_col()
                .w_full()
                .mt(px(resolved.margin_before_or(16.0)))
                .mb(px(resolved.margin_after_or(16.0)))
                .when_some(resolved.background.clone(), |this, background| {
                    this.bg(rgb(parse_hex(&background)))
                })
                .child(image_view(image, &resolved));
            if !caption.is_empty() {
                container = container.child(paragraph_view(caption, &caption_style, theme, false));
            }
            container.into_any_element()
        }
        Node::Table { rows, style } => {
            let mut resolved = theme.style("table");
            if let Some(style) = style {
                resolved = resolved.merged(style);
            }
            let columns = rows.iter().map(|row| row.cells.len()).max().unwrap_or(0);
            div()
                .flex()
                .flex_col()
                .w_full()
                .mt(px(resolved.margin_before_or(12.0)))
                .mb(px(resolved.margin_after_or(12.0)))
                .when_some(resolved.background.clone(), |this, background| {
                    this.bg(rgb(parse_hex(&background)))
                })
                .children(
                    rows.iter()
                        .map(|row| table_row_view(row, theme, Some(&resolved), columns)),
                )
                .into_any_element()
        }
        Node::TableRow { cells, style } => {
            let row = TableRow::new(cells.clone()).with_style(style.clone());
            let inherited = theme.style("table");
            table_row_view(&row, theme, Some(&inherited), cells.len())
        }
        Node::TableCell { blocks, style } => {
            let cell = TableCell::new(blocks.clone()).with_style(style.clone());
            let row_style = theme.style("row");
            table_cell_view(&cell, theme, &row_style)
        }
        Node::Bibliography { entries, style } => {
            let mut resolved = theme.style("bibliography");
            if let Some(style) = style {
                resolved = resolved.merged(style);
            }
            let title_style = theme.heading_style(2);
            let mut entry_style = theme.style("text");
            entry_style.size = Some(13.5);
            entry_style.leading = Some(1.5);
            entry_style.color = Some("#374151".to_string());
            let mut container = div()
                .flex()
                .flex_col()
                .w_full()
                .pt(px(12.0))
                .border_t_1()
                .border_color(rgb(0xd8dee4))
                .mt(px(resolved.margin_before_or(20.0)))
                .mb(px(resolved.margin_after_or(12.0)))
                .when_some(resolved.background.clone(), |this, background| {
                    this.bg(rgb(parse_hex(&background)))
                })
                .child(paragraph_view(
                    &[Inline::Text("参考书目".to_string())],
                    &title_style,
                    theme,
                    true,
                ));
            for (index, entry) in entries.iter().enumerate() {
                let mut inlines = vec![Inline::Text(format!("{}. ", index + 1))];
                inlines.extend(entry.inlines.iter().cloned());
                container = container.child(paragraph_view(&inlines, &entry_style, theme, false));
            }
            container.into_any_element()
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
            div()
                .flex()
                .flex_col()
                .rounded_md()
                .pl(px(resolved.indent_or(14.0)))
                .pr_3()
                .py_2()
                .mt(px(resolved.margin_before_or(10.0)))
                .mb(px(resolved.margin_after_or(10.0)))
                .when_some(resolved.background.clone(), |this, background| {
                    this.bg(rgb(parse_hex(&background)))
                })
                .text_color(rgb(parse_hex(resolved.color_or("#24292f"))))
                .children(
                    blocks
                        .iter()
                        .map(|node| node_view_with(node, theme, Some(&child_style))),
                )
                .into_any_element()
        }
    }
}

fn table_row_view(
    row: &TableRow,
    theme: &Theme,
    inherited: Option<&Style>,
    columns: usize,
) -> AnyElement {
    let mut resolved = theme.style("row");
    if let Some(inherited) = inherited {
        resolved = resolved.merged(&inherited.text_inheritance());
    }
    if let Some(style) = &row.style {
        resolved = resolved.merged(style);
    }
    div()
        .flex()
        .flex_row()
        .items_stretch()
        .w_full()
        .when_some(resolved.background.clone(), |this, background| {
            this.bg(rgb(parse_hex(&background)))
        })
        .children(
            (0..columns.max(row.cells.len())).map(|index| match row.cells.get(index) {
                Some(cell) => table_cell_view(cell, theme, &resolved),
                None => table_cell_view(&TableCell::new(Vec::new()), theme, &resolved),
            }),
        )
        .into_any_element()
}

fn table_cell_view(cell: &TableCell, theme: &Theme, row_style: &Style) -> AnyElement {
    let mut resolved = theme.style("cell").merged(&row_style.text_inheritance());
    if let Some(style) = &cell.style {
        resolved = resolved.merged(style);
    }
    let size = resolved.size_or(15.0);
    let line_height = size * resolved.leading_or(1.6);
    let child_style = resolved.text_inheritance();
    div()
        .flex()
        .flex_col()
        .flex_1()
        .min_w(px(0.0))
        .border_1()
        .border_color(rgb(0xd8dee4))
        .p(px(8.0))
        .text_size(px(size))
        .line_height(px(line_height))
        .text_color(rgb(parse_hex(resolved.color_or("#24292f"))))
        .when_some(resolved.background.clone(), |this, background| {
            this.bg(rgb(parse_hex(&background)))
        })
        .children(
            cell.blocks
                .iter()
                .map(|node| node_view_with(node, theme, Some(&child_style))),
        )
        .into_any_element()
}

fn list_item_view(item: &ListItem, theme: &Theme, inherited: Option<&Style>) -> AnyElement {
    let mut inherited = inherited.cloned();
    if let Some(style) = &item.style {
        inherited = Some(
            inherited
                .map(|inherited| inherited.merged(style))
                .unwrap_or_else(|| style.clone()),
        );
    }
    div()
        .flex()
        .flex_col()
        .children(
            item.blocks
                .iter()
                .map(|node| node_view_with(node, theme, inherited.as_ref())),
        )
        .into_any_element()
}

fn paragraph_view(inlines: &[Inline], style: &Style, theme: &Theme, heading: bool) -> AnyElement {
    let size = style.size_or(15.0);
    let weight = style.weight_or(if heading { 700 } else { 400 });
    let line_height = size * style.leading_or(if heading { 1.35 } else { 1.75 });
    let align = match style.align_or(Align::Left) {
        Align::Left => TextAlign::Left,
        Align::Center => TextAlign::Center,
        Align::Right => TextAlign::Right,
        Align::Justify => TextAlign::Left,
    };
    let mut container = div()
        .flex()
        .flex_col()
        .w_full()
        .mt(px(style.margin_before_or(0.0)))
        .mb(px(style.margin_after_or(10.0)))
        .text_size(px(size))
        .line_height(px(line_height))
        .text_align(align)
        .text_color(rgb(parse_hex(style.color_or("#24292f"))));
    if contains_image(inlines) {
        for segment in paragraph_segments(inlines) {
            match segment {
                ParagraphSegment::Text(inlines) => {
                    if has_visible_text(inlines) {
                        container =
                            container.child(inline_text(inlines, style, theme, weight >= 600));
                    }
                }
                ParagraphSegment::Image(image) => {
                    container = container.child(image_view(image, style));
                }
            }
        }
    } else {
        container = container.child(inline_text(inlines, style, theme, weight >= 600));
    }
    container.into_any_element()
}

enum ParagraphSegment<'a> {
    Text(&'a [Inline]),
    Image(&'a Image),
}

fn paragraph_segments(inlines: &[Inline]) -> Vec<ParagraphSegment<'_>> {
    let mut segments = Vec::new();
    let mut start = 0;
    for (index, inline) in inlines.iter().enumerate() {
        if let Inline::Image(image) = inline {
            if start < index {
                segments.push(ParagraphSegment::Text(&inlines[start..index]));
            }
            segments.push(ParagraphSegment::Image(image));
            start = index + 1;
        }
    }
    if start < inlines.len() {
        segments.push(ParagraphSegment::Text(&inlines[start..]));
    }
    segments
}

fn contains_image(inlines: &[Inline]) -> bool {
    inlines.iter().any(|inline| match inline {
        Inline::Image(_) => true,
        Inline::Strong(children) | Inline::Emphasis(children) => contains_image(children),
        Inline::Link { children, .. } => contains_image(children),
        Inline::Text(_) | Inline::Code(_) | Inline::Reference { .. } | Inline::Break => false,
    })
}

fn has_visible_text(inlines: &[Inline]) -> bool {
    inlines.iter().any(|inline| match inline {
        Inline::Text(text) => !text.trim().is_empty(),
        Inline::Strong(children) | Inline::Emphasis(children) => has_visible_text(children),
        Inline::Link { children, .. } => has_visible_text(children),
        Inline::Code(_) | Inline::Reference { .. } => true,
        Inline::Image(_) | Inline::Break => false,
    })
}

fn image_view(image: &Image, style: &Style) -> AnyElement {
    let mut element = img(PathBuf::from(&image.src)).object_fit(ObjectFit::Contain);
    let has_width = image.width.is_some();
    if let Some(width) = image.width {
        element = match width {
            ImageSize::Percent(value) => element.w(relative(value / 100.0)),
            ImageSize::Points(value) => element.w(px(value * 96.0 / 72.0)),
        };
    }
    if let Some(height) = image.height {
        element = match height {
            ImageSize::Percent(value) => element.h(relative(value / 100.0)),
            ImageSize::Points(value) => element.h(px(value * 96.0 / 72.0)),
        };
    }
    if !has_width && image.height.is_none() {
        element = element.w_full();
    }
    let element = element.max_w(relative(1.0)).into_any_element();
    match style.align_or(Align::Left) {
        Align::Center => div()
            .w_full()
            .flex()
            .justify_center()
            .child(element)
            .into_any_element(),
        Align::Right => div()
            .w_full()
            .flex()
            .justify_end()
            .child(element)
            .into_any_element(),
        Align::Left | Align::Justify => element,
    }
}

fn inline_text(inlines: &[Inline], style: &Style, theme: &Theme, base_bold: bool) -> AnyElement {
    let mut text = String::new();
    let mut runs = Vec::new();
    let base_font = font(
        style
            .font
            .clone()
            .unwrap_or_else(|| "Microsoft YaHei".to_string()),
    );
    let base_font = if base_bold {
        base_font.bold()
    } else {
        base_font
    };
    let base_font = if style.italic.unwrap_or(false) {
        base_font.italic()
    } else {
        base_font
    };
    let base_color: Hsla = rgb(parse_hex(style.color_or("#24292f"))).into();

    append_inlines(
        &mut text, &mut runs, inlines, &base_font, base_color, theme, None,
    );

    if text.is_empty() {
        return div().into_any_element();
    }

    StyledText::new(text).with_runs(runs).into_any_element()
}

fn append_inlines(
    text: &mut String,
    runs: &mut Vec<TextRun>,
    inlines: &[Inline],
    base_font: &gpui::Font,
    base_color: Hsla,
    theme: &Theme,
    background: Option<Hsla>,
) {
    for inline in inlines {
        match inline {
            Inline::Text(value) => {
                push_run(
                    text,
                    runs,
                    value,
                    base_font.clone(),
                    base_color,
                    background,
                    false,
                );
            }
            Inline::Strong(children) => {
                let font = gpui::Font {
                    weight: FontWeight::BOLD,
                    ..base_font.clone()
                };
                append_inlines(text, runs, children, &font, base_color, theme, background);
            }
            Inline::Emphasis(children) => {
                let font = gpui::Font {
                    style: FontStyle::Italic,
                    ..base_font.clone()
                };
                append_inlines(text, runs, children, &font, base_color, theme, background);
            }
            Inline::Code(value) => {
                let code_style = theme.style("code");
                let mono = font(
                    code_style
                        .font
                        .clone()
                        .unwrap_or_else(|| "Cascadia Mono".to_string()),
                );
                let color: Hsla = rgb(parse_hex(code_style.color_or("#b31d28"))).into();
                let bg: Hsla = rgb(parse_hex(
                    code_style.background.as_deref().unwrap_or("#f3f4f6"),
                ))
                .into();
                push_run(text, runs, value, mono, color, Some(bg), false);
            }
            Inline::Link { children, .. } => {
                let color: Hsla = rgb(0x0969da).into();
                append_inlines(text, runs, children, base_font, color, theme, background);
            }
            Inline::Image(image) => {
                push_run(
                    text,
                    runs,
                    &format!("[图片: {}]", image.src),
                    base_font.clone(),
                    base_color,
                    background,
                    false,
                );
            }
            Inline::Reference { index } => {
                let color: Hsla = rgb(0x0969da).into();
                push_run(
                    text,
                    runs,
                    &format!("[{index}]"),
                    base_font.clone(),
                    color,
                    background,
                    false,
                );
            }
            Inline::Break => {
                push_run(
                    text,
                    runs,
                    "\n",
                    base_font.clone(),
                    base_color,
                    background,
                    false,
                );
            }
        }
    }
}

fn push_run(
    text: &mut String,
    runs: &mut Vec<TextRun>,
    value: &str,
    font: gpui::Font,
    color: Hsla,
    background: Option<Hsla>,
    underline: bool,
) {
    if value.is_empty() {
        return;
    }
    text.push_str(value);
    runs.push(TextRun {
        len: value.len(),
        font,
        color,
        background_color: background,
        underline: underline.then_some(UnderlineStyle {
            color: Some(color),
            thickness: px(1.0),
            wavy: false,
        }),
        strikethrough: None,
    });
}

fn parse_hex(hex: &str) -> u32 {
    let hex = hex.trim().trim_start_matches('#');
    match hex.len() {
        3 => {
            let r = u8::from_str_radix(&hex[0..1], 16).unwrap_or(0) * 17;
            let g = u8::from_str_radix(&hex[1..2], 16).unwrap_or(0) * 17;
            let b = u8::from_str_radix(&hex[2..3], 16).unwrap_or(0) * 17;
            ((r as u32) << 16) | ((g as u32) << 8) | b as u32
        }
        6 | 8 => u32::from_str_radix(&hex[0..6], 16).unwrap_or(0x24292f),
        _ => 0x24292f,
    }
}
