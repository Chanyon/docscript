pub mod html;
pub mod pdf;
pub mod preview;
pub mod sample;
pub mod typeset;

use std::path::PathBuf;

use docscript::{Document, compile, compile_with_base};

pub fn compile_source(source: &str) -> Result<Document, String> {
    compile(source).map_err(|errors| {
        errors
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    })
}

pub fn compile_source_with_base(
    source: &str,
    base_dir: Option<PathBuf>,
) -> Result<Document, String> {
    compile_with_base(source, base_dir).map_err(|errors| {
        errors
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_renders_to_html_and_pdf() {
        let example = include_str!("../../../examples/welcome.docscript");
        let example_dir =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples");
        let example_document =
            compile_source_with_base(example, Some(example_dir)).expect("welcome example compiles");
        let example_html = html::render_html(&example_document);
        assert!(
            example_html.contains("data:image/png;base64,"),
            "welcome example should embed its relative image"
        );

        let document = compile_source(crate::sample::SAMPLE).expect("sample compiles");
        let html = html::render_html(&document);
        assert!(html.contains("<!doctype html>"));
        assert!(html.contains("DocScript"));
        assert!(html.contains("custom-note"));
        assert!(html.contains("#b45309"));
        assert!(html.contains("background: #f3f4f6"));

        let pdf = pdf::render_pdf(&document).expect("pdf renders");
        assert!(pdf.starts_with(b"%PDF"));
        assert!(pdf.len() > 1000);

        let mut warnings = Vec::new();
        let parsed = printpdf::PdfDocument::parse(
            &pdf,
            &printpdf::PdfParseOptions::default(),
            &mut warnings,
        )
        .expect("generated pdf parses");
        let text = parsed
            .extract_text()
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join("\n");
        assert!(text.contains("DocScript"), "extracted text: {text:?}");
        assert!(text.contains("文本"), "extracted text: {text:?}");

        let out_dir = std::env::temp_dir().join("docscript-tests");
        std::fs::create_dir_all(&out_dir).unwrap();
        std::fs::write(out_dir.join("sample.pdf"), &pdf).unwrap();
        std::fs::write(out_dir.join("sample.html"), html).unwrap();
    }

    #[test]
    fn images_render_in_paragraphs_for_html_and_pdf() {
        let dir = std::env::temp_dir().join("docscript-image-test");
        std::fs::create_dir_all(&dir).unwrap();
        let image = image::RgbImage::from_fn(8, 5, |x, y| {
            image::Rgb([(x * 28) as u8, (y * 48) as u8, 180])
        });
        let image_path = dir.join("glacier.png");
        image.save(&image_path).unwrap();

        let source = r##"
document {
  h1 { 冰川 }
  p {
    前文
    image("glacier.png", width: 70%)
    后文
  }
  figure {
    image("glacier.png", width: 70%)
    text {
      strong("Glaciers") form an important part
      of the earth's climate system.
    }
  }
}
"##;
        let document =
            compile_source_with_base(source, Some(dir.clone())).expect("image document compiles");
        let html = html::render_html(&document);
        assert!(html.contains("data:image/png;base64,"));
        assert!(html.contains("width: 70%"));
        assert!(html.contains("alt=\"glacier.png\""));
        assert!(html.contains("<figure class=\"figure\""));
        assert!(html.contains("<figcaption class=\"caption\""));
        assert!(html.contains("<strong>Glaciers</strong>"));

        let pdf = pdf::render_pdf(&document).expect("image pdf renders");
        assert!(pdf.starts_with(b"%PDF"));
        let text = String::from_utf8_lossy(&pdf);
        assert!(
            text.contains("/Subtype /Image") || text.contains("/Subtype/Image"),
            "pdf should embed an image xobject"
        );

        let out_dir = std::env::temp_dir().join("docscript-tests");
        std::fs::create_dir_all(&out_dir).unwrap();
        std::fs::write(out_dir.join("image.pdf"), &pdf).unwrap();
        std::fs::write(out_dir.join("image.html"), html).unwrap();
    }
}

#[test]
fn references_render_inline_markers_and_bibliography() {
    let source = r##"
@paper(text { 张三. DocScript 参考书目. 2026. })

document {
  h1 { 参考 }
  p { 本文参考 @paper。 }
}
"##;
    let document = compile_source(source).expect("reference document compiles");
    let html = html::render_html(&document);
    assert!(html.contains("<sup class=\"reference\"><a href=\"#reference-1\">[1]</a></sup>"));
    assert!(html.contains("<section class=\"bibliography\""));
    assert!(html.contains("id=\"reference-1\""));
    assert!(html.contains("张三. DocScript 参考书目. 2026."));

    let pdf = pdf::render_pdf(&document).expect("reference pdf renders");
    assert!(pdf.starts_with(b"%PDF"));
    let mut warnings = Vec::new();
    let parsed =
        printpdf::PdfDocument::parse(&pdf, &printpdf::PdfParseOptions::default(), &mut warnings)
            .expect("reference pdf parses");
    let text = parsed
        .extract_text()
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join("\n");
    assert!(text.contains("[1]"), "extracted text: {text:?}");
    assert!(text.contains("张三"), "extracted text: {text:?}");

    let out_dir = std::env::temp_dir().join("docscript-tests");
    std::fs::create_dir_all(&out_dir).unwrap();
    std::fs::write(out_dir.join("references.pdf"), &pdf).unwrap();
    std::fs::write(out_dir.join("references.html"), html).unwrap();
}

#[test]
fn tables_render_to_html_and_pdf() {
    let source = r##"
@set table(margin_before: 12)
@set row(background: "#f9fafb")
@set cell(background: "#ffffff", size: 13, color: #111827)

document {
  table {
    row {
      cell {
        p { A1 }
        p { A2 }
      }
      cell { p { B1 } }
    }
    row {
      cell { p { C1 } }
      cell { p { D1 } }
    }
  }
}
"##;
    let document = compile_source(source).expect("table document compiles");
    let html = html::render_html(&document);
    assert!(html.contains("<table class=\"table\""));
    assert!(html.contains("<tr class=\"table-row\""));
    assert!(html.contains("<td class=\"table-cell\""));
    assert!(html.contains("A1"));
    assert!(html.contains("A2"));
    assert!(html.contains("D1"));

    let pdf = pdf::render_pdf(&document).expect("table pdf renders");
    assert!(pdf.starts_with(b"%PDF"));
    let mut warnings = Vec::new();
    let parsed =
        printpdf::PdfDocument::parse(&pdf, &printpdf::PdfParseOptions::default(), &mut warnings)
            .expect("table pdf parses");
    let text = parsed
        .extract_text()
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join("\n");
    for expected in ["A1", "A2", "B1", "C1", "D1"] {
        assert!(text.contains(expected), "extracted text: {text:?}");
    }

    let out_dir = std::env::temp_dir().join("docscript-tests");
    std::fs::create_dir_all(&out_dir).unwrap();
    std::fs::write(out_dir.join("table.pdf"), &pdf).unwrap();
    std::fs::write(out_dir.join("table.html"), html).unwrap();
}
