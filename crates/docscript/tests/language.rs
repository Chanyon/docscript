use docscript::{Align, ImageSize, Inline, Node, compile};

const SAMPLE: &str = r##"
let theme = {
  text: { font: "Microsoft YaHei", size: 15, leading: 1.75, color: "#24292f" },
  h1: { size: 30, weight: 700, color: "#111827" },
  h2: { size: 22, weight: 700 }
}

fn note(body) {
  quote { {body} }
}

document(theme, title: "DocScript") {
  h1 { DocScript }
  p {
    这是一个段落，含 bold("粗体")、code("let x = 1") 和 link("链接", "https://example.com")。
    换行也会合并，中文之间不会插入空格。
  }
  list {
    item { 第一项 }
    item { 第二项 }
  }
  note(text { 这是一段引用。 })
  for i in range(1, 4) {
    p("第 ", i, " 段由循环生成。")
  }
  if true {
    p { 条件成立。 }
  }
}
"##;

#[test]
fn parses_and_evaluates_the_language() {
    let document = compile(SAMPLE).expect("sample should compile");
    assert_eq!(document.title, "DocScript");
    assert_eq!(document.theme.style("h1").size, Some(30.0));

    let mut headings = 0;
    let mut paragraphs = 0;
    let mut lists = 0;
    let mut quotes = 0;

    for block in &document.blocks {
        match block {
            Node::Heading { level, .. } => {
                assert_eq!(*level, 1);
                headings += 1;
            }
            Node::Paragraph { .. } => paragraphs += 1,
            Node::List { items, .. } => {
                lists += 1;
                assert_eq!(items.len(), 2);
            }
            Node::Quote { blocks, .. } => {
                quotes += 1;
                assert_eq!(blocks.len(), 1);
            }
            _ => {}
        }
    }

    assert_eq!(headings, 1);
    assert_eq!(paragraphs, 5);
    assert_eq!(lists, 1);
    assert_eq!(quotes, 1);

    let first_paragraph = document
        .blocks
        .iter()
        .find_map(|block| match block {
            Node::Paragraph { inlines, .. } => Some(inlines),
            _ => None,
        })
        .unwrap();

    assert!(
        first_paragraph
            .iter()
            .any(|inline| matches!(inline, Inline::Strong(_)))
    );
    assert!(
        first_paragraph
            .iter()
            .any(|inline| matches!(inline, Inline::Code(code) if code == "let x = 1"))
    );
    assert!(first_paragraph.iter().any(
        |inline| matches!(inline, Inline::Link { target, .. } if target == "https://example.com")
    ));

    let text = first_paragraph
        .iter()
        .map(Inline::plain_text)
        .collect::<String>();
    assert!(text.contains("中文之间不会插入空格"));
}

#[test]
fn reports_helpful_errors() {
    let errors = compile("document { p { 没有闭合 }").unwrap_err();
    assert!(!errors.is_empty());
    assert!(errors[0].line >= 1);
}

#[test]
fn supports_scoped_set_and_defined_elements() {
    let source = r##"
@set text(font: "Source Han Serif", size: 12, color: #333)
@set page(margin: 2cm)
@set heading(level: 1, size: 24, color: #111)
@set h2.style(@style({
  size: 18;
  color: #555;
}))
@set note(background: #fff8e1, indent: 12)
@set h2.font("Source Han Serif")

@define note(body) {
  quote { {body} }
}

document(title: "Scoped") {
  h1 { Title }
  p { Body }
  h2 { Normal H2 }
  @set h2(size: 20, color: #000) {
    h2 { Scoped H2 }
    p { Inside }
  }
  h2 { After Scope }
  note(text { Note body })
}
"##;

    let document = compile(source).expect("scoped set sample compiles");
    assert!((document.theme.page.margin - 20.0).abs() < 0.001);

    let mut headings = Vec::new();
    let mut custom = None;
    let mut body_style = None;
    for block in &document.blocks {
        match block {
            Node::Heading { inlines, style, .. } => {
                headings.push((
                    inlines.iter().map(Inline::plain_text).collect::<String>(),
                    style.clone(),
                ));
            }
            Node::Paragraph { style, .. } => {
                if body_style.is_none() {
                    body_style = style.clone();
                }
            }
            Node::Custom { name, style, .. } if name == "note" => {
                custom = style.clone();
            }
            _ => {}
        }
    }

    let h1_style = headings[0].1.as_ref().expect("h1 style");
    assert_eq!(h1_style.size, Some(24.0));
    assert_eq!(h1_style.color.as_deref(), Some("#111"));

    let normal_h2 = headings[1].1.as_ref().expect("normal h2 style");
    assert_eq!(normal_h2.size, Some(18.0));
    assert_eq!(normal_h2.color.as_deref(), Some("#555"));
    assert_eq!(normal_h2.font.as_deref(), Some("Source Han Serif"));

    let scoped_h2 = headings[2].1.as_ref().expect("scoped h2 style");
    assert_eq!(scoped_h2.size, Some(20.0));
    assert_eq!(scoped_h2.color.as_deref(), Some("#000"));

    let after_h2 = headings[3].1.as_ref().expect("after h2 style");
    assert_eq!(after_h2.size, Some(18.0));

    let body_style = body_style.expect("body style");
    assert_eq!(body_style.size, Some(12.0));
    assert_eq!(body_style.font.as_deref(), Some("Source Han Serif"));

    let custom = custom.expect("note style");
    assert_eq!(custom.background.as_deref(), Some("#fff8e1"));
    assert_eq!(custom.indent, Some(12.0));
}

#[test]
fn raw_backtick_code_blocks_preserve_multiline_code() {
    let source = r##"
document {
  code_block(lang: "c", `
    \\fn main() {
    \\  int a = 1;
    \\}
  `)
}
"##;
    let document = compile(source).expect("raw code block compiles");
    let code = document
        .blocks
        .iter()
        .find_map(|node| match node {
            Node::CodeBlock { text, .. } => Some(text),
            _ => None,
        })
        .expect("code block");
    assert_eq!(code, "fn main() {\n  int a = 1;\n}");
}

#[test]
fn list_items_can_contain_nested_lists() {
    let source = r##"
document {
  list {
    item {
      父项
      list {
        item { 子项一 }
        item { 子项二 }
      }
    }
    item { 第二项 }
  }
}
"##;
    let document = compile(source).expect("nested list compiles");
    eprintln!("{document:#?}");
    let outer = document
        .blocks
        .iter()
        .find_map(|node| match node {
            Node::List { items, .. } => Some(items),
            _ => None,
        })
        .expect("outer list");
    assert_eq!(outer.len(), 2);
    assert_eq!(outer[0].blocks.len(), 2);
    assert!(matches!(outer[0].blocks[0], Node::Paragraph { .. }));
    let inner = match &outer[0].blocks[1] {
        Node::List { items, .. } => items,
        other => panic!("expected nested list, got {other:?}"),
    };
    assert_eq!(inner.len(), 2);
    assert_eq!(inner[0].blocks.len(), 1);
}

#[test]
fn supports_global_free_variables_and_text_interpolation() {
    let source = r##"
let title = "年度报告"
let author = "张三"
let sections = ["导论", "方法", "结论"]

fn signature() {
  p { 作者：{author} }
}

document(title: title) {
  h1 { {title} }
  signature()
  for section in sections {
    h2 { {section} }
    p { 这是{section}部分，作者是{author}。 }
  }
}
"##;
    let document = compile(source).expect("free variables sample compiles");
    assert_eq!(document.title, "年度报告");
    let text = document.plain_text();
    assert!(text.contains("年度报告"));
    assert!(text.contains("作者：张三"));
    assert!(text.contains("这是导论部分，作者是张三。"));
    assert!(text.contains("结论"));
}

#[test]
fn fn_functions_support_set_styles() {
    let source = r##"
@set callout(background: "#fff3cd", color: "#664d03", indent: 12)

fn callout(body) {
  p { {body} }
}

fn plain() {
  p { 普通函数 }
}

document {
  callout(text { 注意 })
  plain()
}
"##;
    let document = compile(source).expect("fn @set sample compiles");
    let callout = document
        .blocks
        .iter()
        .find_map(|node| match node {
            Node::Custom {
                name,
                blocks,
                style,
            } if name == "callout" => Some((blocks, style)),
            _ => None,
        })
        .expect("callout should become a styled custom node");
    assert_eq!(callout.0.len(), 1);
    assert!(matches!(callout.0[0], Node::Paragraph { .. }));
    let style = callout.1.as_ref().expect("callout style");
    assert_eq!(style.background.as_deref(), Some("#fff3cd"));
    assert_eq!(style.color.as_deref(), Some("#664d03"));
    assert_eq!(style.indent, Some(12.0));

    assert!(document.blocks.iter().all(|node| !matches!(
        node,
        Node::Custom { name, .. } if name == "plain"
    )));
}

#[test]
fn images_support_percent_width_inside_paragraphs() {
    let source = r##"
document {
  p {
    冰川照片：
    image("glacier.jpg", width: 70%)
  }
}
"##;
    let document = compile(source).expect("image sample compiles");
    let inlines = document
        .blocks
        .iter()
        .find_map(|node| match node {
            Node::Paragraph { inlines, .. } => Some(inlines),
            _ => None,
        })
        .expect("image paragraph");
    let image = inlines
        .iter()
        .find_map(|inline| match inline {
            Inline::Image(image) => Some(image),
            _ => None,
        })
        .expect("image inline");
    assert_eq!(image.src, "glacier.jpg");
    assert_eq!(image.width, Some(ImageSize::Percent(70.0)));
}

#[test]
fn figures_attach_captions_to_images() {
    let source = r##"
@set figure(align: "center")

document {
  figure {
    image("glacier.jpg", width: 70%)
    text {
      strong("Glaciers") form an important part
      of the earth's climate system.
    }
  }
}
"##;
    let document = compile(source).expect("figure sample compiles");
    let figure = document
        .blocks
        .iter()
        .find_map(|node| match node {
            Node::Figure {
                image,
                caption,
                style,
            } => Some((image, caption, style)),
            _ => None,
        })
        .expect("figure node");
    assert_eq!(figure.0.src, "glacier.jpg");
    assert_eq!(figure.0.width, Some(ImageSize::Percent(70.0)));
    assert_eq!(
        figure.2.as_ref().and_then(|style| style.align),
        Some(Align::Center)
    );
    assert!(matches!(figure.1.first(), Some(Inline::Strong(_))));
    let caption = figure.1.iter().map(Inline::plain_text).collect::<String>();
    assert_eq!(
        caption,
        "Glaciers form an important part of the earth's climate system."
    );
}

#[test]
fn references_build_a_cited_bibliography() {
    let source = r##"
@book(text { 我是书 })
@reference(text { 我是参考书目 })

document {
  p { I want to @reference and @book, again @reference. }
}
"##;
    let document = compile(source).expect("reference sample compiles");
    let paragraph = document
        .blocks
        .iter()
        .find_map(|node| match node {
            Node::Paragraph { inlines, .. } => Some(inlines),
            _ => None,
        })
        .expect("paragraph");
    let refs = paragraph
        .iter()
        .filter_map(|inline| match inline {
            Inline::Reference { index } => Some(*index),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(refs, vec![1, 2, 1]);
    let paragraph_text = paragraph.iter().map(Inline::plain_text).collect::<String>();
    assert_eq!(paragraph_text, "I want to [1] and [2], again [1].");

    let entries = document
        .blocks
        .iter()
        .find_map(|node| match node {
            Node::Bibliography { entries, .. } => Some(entries),
            _ => None,
        })
        .expect("bibliography");
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].name, "reference");
    assert_eq!(
        entries[0]
            .inlines
            .iter()
            .map(Inline::plain_text)
            .collect::<String>(),
        "我是参考书目"
    );
    assert_eq!(entries[1].name, "book");
    assert_eq!(
        entries[1]
            .inlines
            .iter()
            .map(Inline::plain_text)
            .collect::<String>(),
        "我是书"
    );
}

#[test]
fn objects_support_member_access_indexing_and_iteration() {
    let source = r##"
let white = {
  text: { font: "Songti SC", size: 14, leading: 1.6 },
  heading1: { size: 28, weight: 700 },
}

@set text(font: white.text.font, size: white.text.size, leading: white.text.leading)
@set heading(level: 1, size: white.heading1.size, weight: white.heading1.weight)

document {
  h1 { 标题 }
  p { 字体：{white.text.font} }

  for name, value in white {
    p("样式：", name, " 字号：", value.size)
  }

  for name in white {
    p("字段：", name, " 字号：", white[name].size)
  }
}
"##;
    let document = compile(source).expect("object sample compiles");
    let heading = document
        .blocks
        .iter()
        .find_map(|node| match node {
            Node::Heading {
                level: 1, style, ..
            } => Some(style),
            _ => None,
        })
        .expect("h1 style");
    let heading = heading.as_ref().expect("h1 should have @set style");
    assert_eq!(heading.size, Some(28.0));
    assert_eq!(heading.weight, Some(700));

    let paragraph = document
        .blocks
        .iter()
        .find_map(|node| match node {
            Node::Paragraph { style, .. } => Some(style),
            _ => None,
        })
        .expect("paragraph style");
    let paragraph = paragraph
        .as_ref()
        .expect("paragraph should have @set style");
    assert_eq!(paragraph.font.as_deref(), Some("Songti SC"));
    assert_eq!(paragraph.size, Some(14.0));
    assert_eq!(paragraph.leading, Some(1.6));

    let text = document.plain_text();
    assert!(text.contains("字体：Songti SC"));
    assert!(text.contains("样式：heading1 字号：28"));
    assert!(text.contains("样式：text 字号：14"));
    assert!(text.contains("字段：heading1 字号：28"));
    assert!(text.contains("字段：text 字号：14"));
}

#[test]
fn class_styles_apply_to_matching_elements() {
    let source = r##"
@set h2(size: 20)
@set h2.className(@style({
  size: 26;
  color: #b45309;
  margin_before: 10;
}))
@set p.note(color: #0969da)

document {
  h2 { 普通标题 }
  h2.className { 自定义标题 }

  p { 普通段落 }
  p.note { 笔记段落 }
}
"##;
    let document = compile(source).expect("class style sample compiles");
    let headings = document
        .blocks
        .iter()
        .filter_map(|node| match node {
            Node::Heading {
                level: 2,
                inlines,
                style,
            } => Some((inlines, style)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(headings.len(), 2);
    let plain_heading_text = headings[0]
        .0
        .iter()
        .map(Inline::plain_text)
        .collect::<String>();
    assert_eq!(plain_heading_text, "普通标题");
    assert_eq!(
        headings[0].1.as_ref().and_then(|style| style.size),
        Some(20.0)
    );
    let class_heading_text = headings[1]
        .0
        .iter()
        .map(Inline::plain_text)
        .collect::<String>();
    assert_eq!(class_heading_text, "自定义标题");
    let class_heading_style = headings[1].1.as_ref().expect("class heading style");
    assert_eq!(class_heading_style.size, Some(26.0));
    assert_eq!(class_heading_style.color.as_deref(), Some("#b45309"));
    assert_eq!(class_heading_style.margin_before, Some(10.0));

    let paragraphs = document
        .blocks
        .iter()
        .filter_map(|node| match node {
            Node::Paragraph { inlines, style } => Some((inlines, style)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(paragraphs.len(), 2);
    let note_style = paragraphs[1].1.as_ref().expect("note class style");
    assert_eq!(note_style.color.as_deref(), Some("#0969da"));
    let note_text = paragraphs[1]
        .0
        .iter()
        .map(Inline::plain_text)
        .collect::<String>();
    assert_eq!(note_text, "笔记段落");
}

#[test]
fn tables_support_arbitrary_cells_and_styles() {
    let source = r##"
@set table(margin_before: 10)
@set row(background: "#f9fafb")
@set cell(background: "#ffffff", size: 13, color: #111827)
@set cell.emph(background: "#fef3c7")

document {
  table {
    row {
      cell {
        p { A1 }
        p { A2 }
      }
      cell.emph { p { B1 } }
    }
    row {
      cell { p { C1 } }
      cell {
        list {
          item { D1 }
          item { D2 }
        }
      }
    }
  }
}
"##;
    let document = compile(source).expect("table sample compiles");
    let table = document
        .blocks
        .iter()
        .find_map(|node| match node {
            Node::Table { rows, style } => Some((rows, style)),
            _ => None,
        })
        .expect("table node");
    assert_eq!(table.0.len(), 2);
    assert_eq!(
        table.1.as_ref().and_then(|style| style.margin_before),
        Some(10.0)
    );
    let first_row = &table.0[0];
    assert_eq!(first_row.cells.len(), 2);
    assert_eq!(
        first_row
            .style
            .as_ref()
            .and_then(|style| style.background.as_deref()),
        Some("#f9fafb")
    );
    assert_eq!(first_row.cells[0].blocks.len(), 2);
    assert_eq!(
        first_row.cells[0]
            .style
            .as_ref()
            .and_then(|style| style.background.as_deref()),
        Some("#ffffff")
    );
    assert_eq!(
        first_row.cells[1]
            .style
            .as_ref()
            .and_then(|style| style.background.as_deref()),
        Some("#fef3c7")
    );
    assert!(matches!(
        first_row.cells[0].blocks[0],
        Node::Paragraph { .. }
    ));
    assert!(matches!(
        first_row.cells[0].blocks[1],
        Node::Paragraph { .. }
    ));

    let second_row = &table.0[1];
    assert_eq!(second_row.cells.len(), 2);
    assert!(matches!(second_row.cells[1].blocks[0], Node::List { .. }));
}
