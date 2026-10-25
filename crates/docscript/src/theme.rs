use std::collections::BTreeMap;

use crate::value::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Align {
    #[default]
    Left,
    Center,
    Right,
    Justify,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Style {
    pub font: Option<String>,
    pub size: Option<f32>,
    pub weight: Option<u16>,
    pub italic: Option<bool>,
    pub color: Option<String>,
    pub background: Option<String>,
    pub leading: Option<f32>,
    pub align: Option<Align>,
    pub indent: Option<f32>,
    pub margin_before: Option<f32>,
    pub margin_after: Option<f32>,
    pub tracking: Option<f32>,
}

impl Style {
    pub fn merged(&self, child: &Style) -> Style {
        Style {
            font: child.font.clone().or_else(|| self.font.clone()),
            size: child.size.or(self.size),
            weight: child.weight.or(self.weight),
            italic: child.italic.or(self.italic),
            color: child.color.clone().or_else(|| self.color.clone()),
            background: child.background.clone().or_else(|| self.background.clone()),
            leading: child.leading.or(self.leading),
            align: child.align.or(self.align),
            indent: child.indent.or(self.indent),
            margin_before: child.margin_before.or(self.margin_before),
            margin_after: child.margin_after.or(self.margin_after),
            tracking: child.tracking.or(self.tracking),
        }
    }

    pub fn font_or<'a>(&'a self, fallback: &'a str) -> &'a str {
        self.font.as_deref().unwrap_or(fallback)
    }

    pub fn size_or(&self, fallback: f32) -> f32 {
        self.size.unwrap_or(fallback)
    }

    pub fn weight_or(&self, fallback: u16) -> u16 {
        self.weight.unwrap_or(fallback)
    }

    pub fn leading_or(&self, fallback: f32) -> f32 {
        self.leading.unwrap_or(fallback)
    }

    pub fn color_or<'a>(&'a self, fallback: &'a str) -> &'a str {
        self.color.as_deref().unwrap_or(fallback)
    }

    pub fn align_or(&self, fallback: Align) -> Align {
        self.align.unwrap_or(fallback)
    }

    pub fn indent_or(&self, fallback: f32) -> f32 {
        self.indent.unwrap_or(fallback)
    }

    pub fn margin_before_or(&self, fallback: f32) -> f32 {
        self.margin_before.unwrap_or(fallback)
    }

    pub fn margin_after_or(&self, fallback: f32) -> f32 {
        self.margin_after.unwrap_or(fallback)
    }

    pub fn is_empty(&self) -> bool {
        self.font.is_none()
            && self.size.is_none()
            && self.weight.is_none()
            && self.italic.is_none()
            && self.color.is_none()
            && self.background.is_none()
            && self.leading.is_none()
            && self.align.is_none()
            && self.indent.is_none()
            && self.margin_before.is_none()
            && self.margin_after.is_none()
            && self.tracking.is_none()
    }

    /// Properties that text inside a container should inherit.
    pub fn text_inheritance(&self) -> Style {
        Style {
            font: self.font.clone(),
            size: self.size,
            weight: self.weight,
            italic: self.italic,
            color: self.color.clone(),
            background: None,
            leading: self.leading,
            align: self.align,
            indent: None,
            margin_before: None,
            margin_after: None,
            tracking: self.tracking,
        }
    }

    /// Font and color only, used when text inheritance enters a heading.
    pub fn font_color_inheritance(&self) -> Style {
        Style {
            font: self.font.clone(),
            color: self.color.clone(),
            tracking: self.tracking,
            ..Style::default()
        }
    }

    pub fn from_value(value: &Value) -> std::result::Result<Style, String> {
        let Value::Object(object) = value else {
            return Err("样式必须是一个对象".to_string());
        };
        Self::from_pairs(
            object
                .iter()
                .map(|(key, value)| (key.clone(), value.clone())),
        )
    }

    pub fn from_pairs<I>(pairs: I) -> std::result::Result<Style, String>
    where
        I: IntoIterator<Item = (String, Value)>,
    {
        let mut style = Style::default();
        for (key, value) in pairs {
            style.apply_property(&key, &value)?;
        }
        Ok(style)
    }

    pub fn apply_property(&mut self, key: &str, value: &Value) -> std::result::Result<(), String> {
        let key = normalize_property_name(key);
        match key.as_str() {
            "font" | "font-family" => self.font = Some(as_string(value, "font")?),
            "size" | "font-size" => self.size = Some(length_in_pt(value, "size")?),
            "weight" | "font-weight" => {
                self.weight = Some(as_number(value, "weight")?.clamp(0.0, 1000.0) as u16)
            }
            "italic" | "font-style" => {
                self.italic = Some(match value {
                    Value::Bool(flag) => *flag,
                    Value::Str(text) => text != "normal",
                    _ => return Err("`italic` 必须是布尔值或字体样式字符串".to_string()),
                })
            }
            "color" => self.color = Some(as_color(value, "color")?),
            "background" | "background-color" => {
                self.background = Some(as_color(value, "background")?)
            }
            "leading" | "line-height" => {
                let number = as_number(value, "leading")?;
                self.leading = Some(
                    if matches!(value, Value::Dimension(_, unit) if unit == "%") {
                        number / 100.0
                    } else {
                        number
                    },
                );
            }
            "align" | "text-align" => {
                let align = as_string(value, "align")?;
                self.align = Some(match align.as_str() {
                    "left" => Align::Left,
                    "center" => Align::Center,
                    "right" => Align::Right,
                    "justify" => Align::Justify,
                    other => return Err(format!("未知的对齐方式 `{other}`")),
                });
            }
            "indent" | "text-indent" => self.indent = Some(length_in_pt(value, "indent")?),
            "margin" => {
                let value = length_in_pt(value, "margin")?;
                self.margin_before = Some(value);
                self.margin_after = Some(value);
            }
            "margin-before" => self.margin_before = Some(length_in_pt(value, "margin-before")?),
            "margin-after" => self.margin_after = Some(length_in_pt(value, "margin-after")?),
            "tracking" | "letter-spacing" => self.tracking = Some(length_in_pt(value, "tracking")?),
            "style" => {
                let nested = Style::from_value(value)?;
                *self = self.merged(&nested);
            }
            // Selectors are handled before properties are collected.
            "level" | "ordered" => {}
            other => return Err(format!("未知的样式属性 `{other}`")),
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PageStyle {
    pub width: f32,
    pub height: f32,
    pub margin: f32,
}

impl Default for PageStyle {
    fn default() -> Self {
        Self {
            width: 210.0,
            height: 297.0,
            margin: 22.0,
        }
    }
}

impl PageStyle {
    pub fn from_value(value: &Value) -> std::result::Result<Self, String> {
        let Value::Object(object) = value else {
            return Err("page 必须是一个对象".to_string());
        };
        let mut page = Self::default();
        page.apply_pairs(
            object
                .iter()
                .map(|(key, value)| (key.clone(), value.clone())),
        )?;
        Ok(page)
    }

    pub fn apply_pairs<I>(&mut self, pairs: I) -> std::result::Result<(), String>
    where
        I: IntoIterator<Item = (String, Value)>,
    {
        for (key, value) in pairs {
            match normalize_property_name(&key).as_str() {
                "width" => self.width = length_in_mm(&value, "page.width")?,
                "height" => self.height = length_in_mm(&value, "page.height")?,
                "margin" => self.margin = length_in_mm(&value, "page.margin")?,
                other => return Err(format!("未知的页面属性 `{other}`")),
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Theme {
    pub base: Style,
    pub styles: BTreeMap<String, Style>,
    pub page: PageStyle,
}

impl Default for Theme {
    fn default() -> Self {
        let base = Style {
            font: Some("Microsoft YaHei".to_string()),
            size: Some(15.0),
            weight: Some(400),
            italic: Some(false),
            color: Some("#24292f".to_string()),
            background: None,
            leading: Some(1.75),
            align: Some(Align::Left),
            indent: None,
            margin_before: Some(0.0),
            margin_after: Some(10.0),
            tracking: Some(0.0),
        };

        let mut styles = BTreeMap::new();
        styles.insert(
            "h1".to_string(),
            Style {
                size: Some(32.0),
                weight: Some(700),
                color: Some("#111827".to_string()),
                leading: Some(1.3),
                margin_before: Some(0.0),
                margin_after: Some(18.0),
                ..Style::default()
            },
        );
        styles.insert(
            "h2".to_string(),
            Style {
                size: Some(24.0),
                weight: Some(700),
                color: Some("#111827".to_string()),
                leading: Some(1.35),
                margin_before: Some(28.0),
                margin_after: Some(12.0),
                ..Style::default()
            },
        );
        styles.insert(
            "h3".to_string(),
            Style {
                size: Some(19.0),
                weight: Some(650),
                color: Some("#1f2937".to_string()),
                leading: Some(1.4),
                margin_before: Some(22.0),
                margin_after: Some(10.0),
                ..Style::default()
            },
        );
        styles.insert("h4".to_string(), styles["h3"].clone());
        styles.insert("h5".to_string(), styles["h3"].clone());
        styles.insert("h6".to_string(), styles["h3"].clone());
        styles.insert(
            "code".to_string(),
            Style {
                font: Some("Cascadia Mono".to_string()),
                size: Some(13.0),
                color: Some("#b31d28".to_string()),
                background: Some("#f3f4f6".to_string()),
                ..Style::default()
            },
        );
        styles.insert(
            "code_block".to_string(),
            Style {
                font: Some("Cascadia Mono".to_string()),
                size: Some(13.0),
                color: Some("#24292f".to_string()),
                background: Some("#f6f8fa".to_string()),
                leading: Some(1.55),
                margin_before: Some(12.0),
                margin_after: Some(12.0),
                ..Style::default()
            },
        );
        styles.insert(
            "quote".to_string(),
            Style {
                color: Some("#57606a".to_string()),
                indent: Some(18.0),
                margin_before: Some(12.0),
                margin_after: Some(12.0),
                ..Style::default()
            },
        );
        styles.insert(
            "list".to_string(),
            Style {
                indent: Some(24.0),
                margin_before: Some(8.0),
                margin_after: Some(8.0),
                ..Style::default()
            },
        );
        styles.insert(
            "figure".to_string(),
            Style {
                align: Some(Align::Center),
                margin_before: Some(16.0),
                margin_after: Some(16.0),
                ..Style::default()
            },
        );
        styles.insert(
            "table".to_string(),
            Style {
                margin_before: Some(12.0),
                margin_after: Some(12.0),
                ..Style::default()
            },
        );
        styles.insert(
            "caption".to_string(),
            Style {
                size: Some(13.0),
                color: Some("#6b7280".to_string()),
                ..Style::default()
            },
        );

        Self {
            base,
            styles,
            page: PageStyle::default(),
        }
    }
}

impl Theme {
    pub fn style(&self, name: &str) -> Style {
        match self.styles.get(name) {
            Some(style) => self.base.merged(style),
            None => self.base.clone(),
        }
    }

    pub fn heading_style(&self, level: u8) -> Style {
        self.style(&format!("h{}", level.clamp(1, 6)))
    }

    pub fn from_value(value: &Value) -> std::result::Result<Theme, String> {
        let Value::Object(object) = value else {
            return Err("theme 必须是一个对象，例如 { text: { size: 15 } }".to_string());
        };

        let mut theme = Theme::default();
        for (key, value) in object.iter() {
            if key == "page" {
                theme.page = PageStyle::from_value(value)?;
                continue;
            }
            let style = Style::from_value(value)?;
            if key == "text" {
                theme.base = theme.base.merged(&style);
            } else {
                theme.styles.insert(key.clone(), style);
            }
        }
        Ok(theme)
    }
}

fn normalize_property_name(name: &str) -> String {
    name.trim().replace('_', "-").to_ascii_lowercase()
}

fn as_string(value: &Value, name: &str) -> std::result::Result<String, String> {
    match value {
        Value::Str(text) => Ok(text.clone()),
        _ => Err(format!("`{name}` 必须是字符串")),
    }
}

fn as_color(value: &Value, name: &str) -> std::result::Result<String, String> {
    as_string(value, name)
}

fn as_number(value: &Value, name: &str) -> std::result::Result<f32, String> {
    match value {
        Value::Int(number) => Ok(*number as f32),
        Value::Float(number) => Ok(*number as f32),
        Value::Dimension(number, _) => Ok(*number),
        _ => Err(format!("`{name}` 必须是数字")),
    }
}

fn length_in_pt(value: &Value, name: &str) -> std::result::Result<f32, String> {
    match value {
        Value::Int(number) => Ok(*number as f32),
        Value::Float(number) => Ok(*number as f32),
        Value::Dimension(number, unit) => convert_length(*number, unit, name),
        Value::Str(text) => parse_length_text(text, name),
        _ => Err(format!("`{name}` 必须是长度（例如 12、12pt、2cm）")),
    }
}

fn length_in_mm(value: &Value, name: &str) -> std::result::Result<f32, String> {
    match value {
        Value::Int(number) => Ok(*number as f32),
        Value::Float(number) => Ok(*number as f32),
        Value::Dimension(number, unit) => convert_length_mm(*number, unit, name),
        Value::Str(text) => parse_length_text_mm(text, name),
        _ => Err(format!("`{name}` 必须是长度（例如 210、210mm、2cm）")),
    }
}

fn convert_length_mm(number: f32, unit: &str, name: &str) -> std::result::Result<f32, String> {
    match unit {
        "mm" => Ok(number),
        "cm" => Ok(number * 10.0),
        "pt" => Ok(number * 0.352_778),
        "px" => Ok(number * 0.264_583),
        "em" | "rem" => Ok(number * 4.233_333),
        "%" => Ok(number),
        other => Err(format!("`{name}` 不支持单位 `{other}`")),
    }
}

fn parse_length_text_mm(text: &str, name: &str) -> std::result::Result<f32, String> {
    let split = text
        .find(|ch: char| !(ch.is_ascii_digit() || ch == '.' || ch == '-' || ch == '+'))
        .unwrap_or(text.len());
    let (number, unit) = text.split_at(split);
    let number = number
        .trim()
        .parse::<f32>()
        .map_err(|_| format!("无法解析 `{name}` 的长度 `{text}`"))?;
    if unit.is_empty() {
        Ok(number)
    } else {
        convert_length_mm(number, &unit.to_ascii_lowercase(), name)
    }
}

fn convert_length(number: f32, unit: &str, name: &str) -> std::result::Result<f32, String> {
    match unit {
        "pt" => Ok(number),
        "px" => Ok(number * 0.75),
        "cm" => Ok(number * 28.346),
        "mm" => Ok(number * 2.8346),
        "em" | "rem" => Ok(number * 12.0),
        "%" => Ok(number),
        other => Err(format!("`{name}` 不支持单位 `{other}`")),
    }
}

fn parse_length_text(text: &str, name: &str) -> std::result::Result<f32, String> {
    let split = text
        .find(|ch: char| !(ch.is_ascii_digit() || ch == '.' || ch == '-' || ch == '+'))
        .unwrap_or(text.len());
    let (number, unit) = text.split_at(split);
    let number = number
        .trim()
        .parse::<f32>()
        .map_err(|_| format!("无法解析 `{name}` 的长度 `{text}`"))?;
    if unit.is_empty() {
        Ok(number)
    } else {
        convert_length(number, &unit.to_ascii_lowercase(), name)
    }
}
