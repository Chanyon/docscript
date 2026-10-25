use std::path::PathBuf;

use docscript::{Document, compile_with_base};
use docscript_editor::{html, pdf, preview, sample};
use gpui::{
    AnyElement, App, Bounds, ClickEvent, Context, Entity, KeyBinding, SharedString, Window,
    WindowBounds, WindowOptions, div, prelude::*, px, size,
};
use gpui_kit::component::{
    ActiveTheme as _, Sizable as _, Theme, ThemeMode,
    button::{Button, ButtonVariants as _},
    input::{Editor, EditorState, InputEvent},
    separator::Separator,
    status_bar::StatusBar,
};

gpui_kit::actions!(
    docscript_app,
    [
        OpenFile,
        SaveFile,
        ExportHtml,
        ExportPdf,
        ResetSample,
        ToggleTheme
    ]
);

struct DocScriptApp {
    editor: Entity<EditorState>,
    source: String,
    document: Option<Document>,
    diagnostics: Vec<String>,
    status: SharedString,
    current_path: Option<PathBuf>,
}

impl DocScriptApp {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let editor = cx.new(|cx| {
            EditorState::new(window, cx)
                .default_value(sample::SAMPLE)
                .language("plaintext")
        });

        let mut app = Self {
            editor: editor.clone(),
            source: sample::SAMPLE.to_string(),
            document: None,
            diagnostics: Vec::new(),
            status: "正在编译…".into(),
            current_path: None,
        };
        app.recompile();

        cx.subscribe(&editor, |this, _editor, event, cx| {
            if matches!(event, InputEvent::Change) {
                let source = this.editor.read(cx).value().to_string();
                if this.source != source {
                    this.source = source;
                    this.recompile();
                    cx.notify();
                }
            }
        })
        .detach();

        app
    }

    fn image_base_dir(&self) -> Option<PathBuf> {
        self.current_path
            .as_ref()
            .and_then(|path| path.parent())
            .map(PathBuf::from)
    }

    fn recompile(&mut self) {
        match compile_with_base(&self.source, self.image_base_dir()) {
            Ok(document) => {
                self.status = format!("编译通过 · {} 个块", document.blocks.len()).into();
                self.document = Some(document);
                self.diagnostics.clear();
            }
            Err(errors) => {
                self.diagnostics = errors.iter().map(ToString::to_string).collect();
                if let Some(first) = self.diagnostics.first() {
                    self.status = format!("编译失败 · {first}").into();
                }
            }
        }
    }

    fn set_editor_text(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.editor
            .update(cx, |editor, cx| editor.set_value(text, window, cx));
        self.editor
            .update(cx, |editor, cx| editor.focus(window, cx));
        self.source = text.to_string();
        self.recompile();
        cx.notify();
    }

    fn open_file(&mut self, _: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.do_open_file(window, cx);
    }

    fn open_file_action(&mut self, _: &OpenFile, window: &mut Window, cx: &mut Context<Self>) {
        self.do_open_file(window, cx);
    }

    fn do_open_file(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("DocScript", &["docscript", "ds", "txt"])
            .pick_file()
        else {
            return;
        };
        match std::fs::read_to_string(&path) {
            Ok(source) => {
                self.current_path = Some(path.clone());
                self.set_editor_text(&source, window, cx);
                self.status = format!("�Ѵ򿪣�{}", path.display()).into();
            }
            Err(error) => {
                self.status = format!("打开失败：{error}").into();
            }
        }
        cx.notify();
    }

    fn save_file(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.do_save_file(cx);
    }

    fn save_file_action(&mut self, _: &SaveFile, _: &mut Window, cx: &mut Context<Self>) {
        self.do_save_file(cx);
    }

    fn do_save_file(&mut self, cx: &mut Context<Self>) {
        let path = if let Some(path) = self.current_path.clone() {
            Some(path)
        } else {
            rfd::FileDialog::new()
                .add_filter("DocScript", &["docscript"])
                .set_file_name("document.docscript")
                .save_file()
        };
        let Some(path) = path else {
            return;
        };
        match std::fs::write(&path, self.source.as_bytes()) {
            Ok(()) => {
                self.current_path = Some(path.clone());
                self.recompile();
                self.status = format!("�ѱ��棺{}", path.display()).into();
            }
            Err(error) => {
                self.status = format!("保存失败：{error}").into();
            }
        }
        cx.notify();
    }

    fn export_html(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.do_export_html(cx);
    }

    fn export_html_action(&mut self, _: &ExportHtml, _: &mut Window, cx: &mut Context<Self>) {
        self.do_export_html(cx);
    }

    fn do_export_html(&mut self, cx: &mut Context<Self>) {
        let Some(document) = self.document.as_ref() else {
            self.status = "文档尚未编译通过，无法导出".into();
            cx.notify();
            return;
        };
        let path = export_path(&document.title, "html");
        let html = html::render_html(document);
        match std::fs::write(&path, html) {
            Ok(()) => self.status = format!("已导出 HTML：{}", path.display()).into(),
            Err(error) => self.status = format!("导出 HTML 失败：{error}").into(),
        }
        cx.notify();
    }

    fn export_pdf(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.do_export_pdf(cx);
    }

    fn export_pdf_action(&mut self, _: &ExportPdf, _: &mut Window, cx: &mut Context<Self>) {
        self.do_export_pdf(cx);
    }

    fn do_export_pdf(&mut self, cx: &mut Context<Self>) {
        let Some(document) = self.document.clone() else {
            self.status = "文档尚未编译通过，无法导出".into();
            cx.notify();
            return;
        };
        let path = export_path(&document.title, "pdf");
        self.status = format!("正在导出 PDF：{}", path.display()).into();
        cx.notify();

        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move { pdf::render_pdf(&document) })
                .await;
            this.update(cx, |this, cx| {
                match result {
                    Ok(bytes) => match std::fs::write(&path, bytes) {
                        Ok(()) => {
                            this.status = format!("已导出 PDF：{}", path.display()).into();
                        }
                        Err(error) => {
                            this.status = format!("写入 PDF 失败：{error}").into();
                        }
                    },
                    Err(error) => {
                        this.status = format!("导出 PDF 失败：{error}").into();
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn reset_sample(&mut self, _: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.current_path = None;
        self.set_editor_text(sample::SAMPLE, window, cx);
        self.status = "已重置为示例文档".into();
        cx.notify();
    }

    fn reset_sample_action(
        &mut self,
        _: &ResetSample,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.current_path = None;
        self.set_editor_text(sample::SAMPLE, window, cx);
        self.status = "已重置为示例文档".into();
        cx.notify();
    }

    fn toggle_theme(&mut self, _: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.do_toggle_theme(window, cx);
    }

    fn toggle_theme_action(
        &mut self,
        _: &ToggleTheme,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.do_toggle_theme(window, cx);
    }

    fn do_toggle_theme(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let next = if cx.theme().mode.is_dark() {
            ThemeMode::Light
        } else {
            ThemeMode::Dark
        };
        Theme::change(next, Some(window), cx);
        cx.notify();
    }

    fn toolbar(&self, cx: &mut Context<Self>) -> AnyElement {
        let muted = cx.theme().muted_foreground;
        let foreground = cx.theme().foreground;

        div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .h(px(56.0))
            .px_4()
            .bg(cx.theme().title_bar)
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .text_size(px(18.0))
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_color(foreground)
                            .child("DocScript"),
                    )
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(muted)
                            .child("用代码写文档 · GPUI-KIT"),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .child(
                        Button::new("open-file")
                            .label("打开")
                            .small()
                            .on_click(cx.listener(Self::open_file)),
                    )
                    .child(
                        Button::new("save-file")
                            .label("保存")
                            .small()
                            .on_click(cx.listener(Self::save_file)),
                    )
                    .child(Separator::vertical().h(px(22.0)))
                    .child(
                        Button::new("export-html")
                            .label("导出 HTML")
                            .small()
                            .on_click(cx.listener(Self::export_html)),
                    )
                    .child(
                        Button::new("export-pdf")
                            .label("导出 PDF")
                            .small()
                            .primary()
                            .on_click(cx.listener(Self::export_pdf)),
                    )
                    .child(Separator::vertical().h(px(22.0)))
                    .child(
                        Button::new("toggle-theme")
                            .label(if cx.theme().mode.is_dark() {
                                "浅色"
                            } else {
                                "深色"
                            })
                            .small()
                            .ghost()
                            .on_click(cx.listener(Self::toggle_theme)),
                    )
                    .child(
                        Button::new("reset-sample")
                            .label("重置示例")
                            .small()
                            .ghost()
                            .on_click(cx.listener(Self::reset_sample)),
                    ),
            )
            .into_any_element()
    }

    fn pane_header(&self, label: &'static str, cx: &mut Context<Self>) -> AnyElement {
        div()
            .flex()
            .flex_row()
            .items_center()
            .h(px(32.0))
            .px_3()
            .bg(cx.theme().tab_bar)
            .border_b_1()
            .border_color(cx.theme().border)
            .text_size(px(12.0))
            .text_color(cx.theme().muted_foreground)
            .child(label)
            .into_any_element()
    }

    fn preview_pane(&self, cx: &mut Context<Self>) -> AnyElement {
        let body = if let Some(document) = &self.document {
            preview::document_view(document)
        } else {
            div()
                .p_8()
                .text_color(cx.theme().muted_foreground)
                .child("还没有可预览的文档。")
                .into_any_element()
        };

        div()
            .flex()
            .flex_col()
            .items_center()
            .w_full()
            .p(px(26.0))
            .child(body)
            .into_any_element()
    }

    fn status_bar(&self, cx: &mut Context<Self>) -> AnyElement {
        let failed = !self.diagnostics.is_empty();
        let color = if failed {
            cx.theme().danger
        } else {
            cx.theme().success
        };
        let text = if failed {
            self.diagnostics
                .iter()
                .take(3)
                .cloned()
                .collect::<Vec<_>>()
                .join("    ")
        } else {
            self.status.to_string()
        };

        StatusBar::new()
            .left(div().text_color(color).child(text))
            .right("UTF-8 · GPUI-KIT")
            .into_any_element()
    }
}

impl Render for DocScriptApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(cx.theme().background)
            .on_action(cx.listener(Self::open_file_action))
            .on_action(cx.listener(Self::save_file_action))
            .on_action(cx.listener(Self::export_html_action))
            .on_action(cx.listener(Self::export_pdf_action))
            .on_action(cx.listener(Self::reset_sample_action))
            .on_action(cx.listener(Self::toggle_theme_action))
            .child(self.toolbar(cx))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_1()
                    .overflow_hidden()
                    .child(
                        div()
                            .w(px(0.0))
                            .flex_1()
                            .h_full()
                            .flex()
                            .flex_col()
                            .child(self.pane_header("源码", cx))
                            .child(div().flex_1().overflow_hidden().p_2().child(
                                Editor::new(&self.editor).h_full().w_full().bordered(false),
                            )),
                    )
                    .child(Separator::vertical())
                    .child(
                        div()
                            .w(px(0.0))
                            .flex_1()
                            .h_full()
                            .flex()
                            .flex_col()
                            .child(self.pane_header("文档预览", cx))
                            .child(
                                div()
                                    .flex_1()
                                    .id("preview-scroll")
                                    .overflow_scroll()
                                    .bg(cx.theme().muted)
                                    .child(self.preview_pane(cx)),
                            ),
                    ),
            )
            .child(self.status_bar(cx))
    }
}

fn export_path(title: &str, extension: &str) -> PathBuf {
    let dir = std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("exports");
    let _ = std::fs::create_dir_all(&dir);
    dir.join(format!("{}.{}", sanitize_filename(title), extension))
}

fn sanitize_filename(title: &str) -> String {
    let mut out = String::new();
    for ch in title.chars() {
        if ch.is_alphanumeric() || ch == '-' || ch == '_' || ch == ' ' || is_cjk(ch) {
            out.push(ch);
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    let trimmed = out.trim_matches(&[' ', '-'][..]).to_string();
    if trimmed.is_empty() {
        "document".to_string()
    } else {
        trimmed
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

fn main() {
    gpui_kit::application().run(|cx: &mut App| {
        gpui_kit::init(cx);
        Theme::change(ThemeMode::Dark, None, cx);
        cx.bind_keys([
            KeyBinding::new("secondary-o", OpenFile, None),
            KeyBinding::new("secondary-s", SaveFile, None),
            KeyBinding::new("secondary-shift-s", ExportPdf, None),
            KeyBinding::new("secondary-t", ToggleTheme, None),
        ]);

        let bounds = Bounds::centered(None, size(px(1280.0), px(860.0)), cx);
        let (_window, _view) = gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            cx,
            |window, cx| {
                window.set_window_title("DocScript · GPUI-KIT");
                let view = cx.new(|cx| DocScriptApp::new(window, cx));
                let editor = view.read(cx).editor.clone();
                editor.update(cx, |editor, cx| editor.focus(window, cx));
                view
            },
        )
        .expect("failed to open DocScript window");
        cx.activate(true);
    });
}
