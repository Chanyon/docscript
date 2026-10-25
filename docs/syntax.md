# DocScript 语法文档

DocScript 是一门“用代码写文档”的语言。文档结构由显式的函数调用组成，正文写在文本块里，同时支持变量、函数、条件、循环、样式、图片、图注、表格和参考书目。

```docscript
let title = "年度报告"
let author = "张三"

document(title: title) {
  h1 { {title} }
  p { 作者：{author} }

  for section in ["导论", "方法", "结论"] {
    h2 { {section} }
    p { 这是{section}部分。 }
  }
}
```

---

## 1. 词法与注释

- 注释使用 `//`，只在代码区域生效；文本块里的 `//` 是普通文本。
- 语句通常用换行分隔，分号 `;` 可选。
- 标识符用于变量、函数、参数、对象字段和参考名。
- 文本块中的 `{`、`}`、`@`、反斜杠，以及紧跟标识符的 `(` 具有特殊含义；需要字面量时可使用反斜杠转义。

```docscript
// 这是代码注释
let name = "DocScript"

p { 文本中的 \{ 和 \} 会原样显示。 }
p { 字面量 \@name 不会变成引用。 }
```

---

## 2. 程序结构

一个 `.docscript` 文件可以包含以下顶层语句：

```docscript
let x = 1
fn greet(name) { p("你好，", name) }
@define note(body) { quote { {body} } }
@reference(text { 参考文献内容 })
@set text(size: 15)
if x > 0 { p { 正数 } } else { p { 非正数 } }
for i in range(3) { p("第 ", i, " 项") }
document { p { 正文 } }
```

语句类型：

| 语法 | 作用 |
| --- | --- |
| `let name = 表达式` | 定义变量 |
| `fn name(参数) { ... }` | 定义普通函数 |
| `@define name(参数) { ... }` | 定义自定义语义元素 |
| `@name(内容)` | 定义参考书目条目，`name` 可自定义 |
| `@set ...` | 定义样式 |
| `if 条件 { ... } else { ... }` | 条件执行 |
| `for ... in ... { ... }` | 迭代 |
| `表达式` | 求值，生成的节点会进入文档 |

---

## 3. 文档入口

使用 `document` 创建文档：

```docscript
document(theme, title: "文档标题") {
  h1 { 标题 }
  p { 正文 }
}
```

- 第一个参数通常传入主题对象。
- `title:` 设置文档标题。
- `document` 的花括号体是文档正文。
- 没有调用 `document` 时，顶层生成的内容会被收集成匿名文档。

### 主题对象

```docscript
let theme = {
  text: {
    font: "Microsoft YaHei",
    size: 15,
    leading: 1.75,
    color: "#24292f"
  },
  h1: { size: 32, weight: 700, color: "#111827" },
  h2: { size: 24, weight: 700, margin_before: 28 },
  page: { width: 210, height: 297, margin: 22 }
}

document(theme, title: "示例") {
  h1 { 示例标题 }
  p { 正文 }
}
```

主题规则：

- `text` 会合并到全局基础文字样式。
- `page` 配置页面宽高和页边距，数值单位默认是毫米（mm）。
- 其他字段是命名样式，例如 `h1`、`h2`、`quote`、`code`、`code_block`、`caption`、`figure`。

---

## 4. 值类型

| 类型 | 示例 |
| --- | --- |
| Null | `null` |
| Bool | `true`、`false` |
| Int | `42`、`-3` |
| Float | `1.6`、`3.14` |
| Dimension | `12pt`、`2cm`、`70%` |
| String | `"hello"` |
| List | `["导论", "方法", "结论"]` |
| Object | `{ size: 14, font: "Songti SC" }` |
| Inlines | `bold("重点")` 的返回值 |
| Nodes | `p { ... }` 的返回值 |
| Function | `fn hello(name) { ... }` 的返回值 |
| Document | `document { ... }` 的返回值 |

### 字符串转义

字符串支持：

- `\n` 换行
- `\r` 回车
- `\t` 制表符
- `\"` 双引号
- `\\` 反斜杠
- `\u{1F600}` Unicode 码点

### 对象与成员

```docscript
let white = {
  text: { font: "Songti SC", size: 14, leading: 1.6 },
  heading1: { size: 28, weight: 700 },
}

p { 字体是 {white.text.font} }
p { 一级标题字号是 {white.heading1.size} }
```

成员和索引：

```docscript
white.text.font        // "Songti SC"
white["text"]["size"]  // 14
white["heading1"].size // 28
white.length           // 对象字段数量
```

- 对象字段不存在会报错。
- 列表支持整数索引，负数从末尾开始。
- 字符串支持整数索引，返回单个字符。
- 列表、字符串和对象支持 `.length` / `.len`。

### 列表

```docscript
let sections = ["导论", "方法", "结论"]

p { 第一项：{sections[0]} }
p { 最后一项：{sections[-1]} }
p { 共 {sections.length} 项 }
```

### 对象迭代

```docscript
for key in white {
  p("字段名：", key)
}

for key, value in white {
  p("字段：", key, "，字号：", value.size)
}
```

- `for key in object`：迭代对象键。
- `for key, value in object`：迭代键值对。
- 当前对象迭代顺序按字段名排序。

列表迭代：

```docscript
for value in sections {
  p("值：", value)
}

for index, value in sections {
  p("第 ", index, " 项：", value)
}
```

---

## 5. 表达式与运算符

### 运算符

从高到低：

| 运算符 | 说明 |
| --- | --- |
| `.`、`[]` | 成员访问、索引 |
| `!`、`-` | 逻辑非、数值取负 |
| `*`、`/`、`%` | 乘、除、取余 |
| `+`、`-` | 加、减；字符串和值可用 `+` 连接 |
| `..`、`..=` | 范围，右端开区间/闭区间 |
| `<`、`<=`、`>`、`>=` | 比较 |
| `==`、`!=` | 相等、不等 |
| `&&` | 逻辑与，短路 |
| `\|\|` | 逻辑或，短路 |

示例：

```docscript
p { 2 + 3 * 4 = {2 + 3 * 4} }
p { 是否相等：{2 + 2 == 4} }

for i in 1..4 {
  p("i = ", i) // 1, 2, 3
}

for i in 1..=3 {
  p("i = ", i) // 1, 2, 3
}
```

### 列表和对象字面量

```docscript
let numbers = [1, 2, 3]
let config = {
  font: "Songti SC",
  size: 14,
  nested: { weight: 700 }
}
```

对象字段分隔符可使用逗号 `,` 或分号 `;`，末尾分隔符可省略或保留。

### 函数调用

```docscript
greet("世界")
greet(name: "世界")
```

- 参数可以按位置传入，也可以按名称传入。
- 函数参数支持默认值。

```docscript
fn greet(name = "世界") {
  p("你好，", name)
}
```

---

## 6. 变量、函数和自定义元素

### 变量

```docscript
let title = "年度报告"
let author = "张三"
let sections = ["导论", "方法", "结论"]
```

顶层 `let` 是全局自由变量。函数可以引用其定义环境中可见的变量：

```docscript
let author = "张三"

fn signature() {
  p { 作者：{author} }
}

document {
  signature()
}
```

### 普通函数

```docscript
fn heading(text) {
  h2 { {text} }
}

fn signature() {
  p { 作者：{author} }
}

document {
  heading("方法")
  signature()
}
```

普通函数可以使用参数、默认值、命名参数和自由变量。函数返回最后一个表达式生成的节点或值。

### `@define` 自定义语义元素

```docscript
@define note(body) {
  quote { {body} }
}

@set note(background: "#f3f4f6", indent: 16, color: "#374151")

document {
  note(text { 这是一段注意内容。 })
}
```

- `@define` 创建自定义元素，并始终把函数输出包装成自定义节点。
- 可以用 `@set 元素名(...)` 设置它的容器样式。

### 给 `fn` 函数设置样式

```docscript
fn callout(body) {
  p { {body} }
}

@set callout(background: "#fff3cd", indent: 12, color: "#664d03")

document {
  callout(text { 注意 })
}
```

当函数返回块内容且存在匹配的 `@set 函数名(...)` 时，输出会被包装为可设置样式的自定义容器。返回内联内容的函数仍保持内联。

---

## 7. 条件与循环

### `if`

```docscript
if true {
  p { 条件成立 }
}

if author == "张三" {
  p { 作者匹配 }
} else {
  p { 作者不匹配 }
}
```

支持 `else if`：

```docscript
if x > 0 {
  p { 正数 }
} else if x == 0 {
  p { 零 }
} else {
  p { 负数 }
}
```

### `for`

```docscript
for item in ["A", "B", "C"] {
  p(item)
}

for i in range(1, 4) {
  p("第 ", i, " 项")
}
```

迭代形式：

| 写法 | 行为 |
| --- | --- |
| `for value in list` | 遍历列表值 |
| `for index, value in list` | 遍历索引和值 |
| `for char in "abc"` | 遍历字符 |
| `for index, char in "abc"` | 遍历字符索引和字符 |
| `for n in 3` | 遍历 0..3 |
| `for key in object` | 遍历对象键 |
| `for key, value in object` | 遍历对象键值对 |

---

## 8. 文本块与内联语法

以下函数使用文本块：

```text
p
paragraph
h1 h2 h3 h4 h5 h6
heading
title
subtitle
quote
item
text
span
bold
strong
italic
em
code
link
caption
label
```

### 文本块

```docscript
p {
  这是正文。
  这里可以直接写中文和英文。
}
```

### 插值

文本块里的 `{表达式}` 会求值：

```docscript
let name = "DocScript"
let size = 14

p { 名称：{name} }
p { 2 + 3 = {2 + 3} }
p { 字号：{size}pt }
```

插值结果可以包含格式化内容：

```docscript
p {
  这是 {bold("加粗")} 和 {code("inline code")}。
}
```

### 内联函数

| 函数 | 示例 | 作用 |
| --- | --- | --- |
| `bold` / `strong` | `bold("重点")` | 加粗 |
| `italic` / `em` | `italic("强调")` | 斜体 |
| `code` | `code("let x = 1")` | 行内代码 |
| `link` | `link("链接", "https://example.com")` | 链接 |
| `text` / `span` | `text { 内容 }` | 保持内联内容 |
| `br` / `line_break` | `br()` | 换行 |
| `image` / `img` | `image("glacier.jpg", width: 70%)` | 图片 |

示例：

```docscript
p {
  访问 {link("示例站点", "https://example.com")}，
  或者查看 {code("docs/syntax.md")}。
}
```

### 转义

文本块中：

| 写法 | 结果 |
| --- | --- |
| `\{` | 字面量 `{` |
| `\}` | 字面量 `}` |
| `\(` | 字面量 `(` |
| `\)` | 字面量 `)` |
| `\@` | 字面量 `@` |
| `\\` | 字面量反斜杠 |

### 文本块中的嵌套列表

`item` 的文本中可以嵌套 `list`，嵌套 `list` 必须独占行首：

```docscript
list {
  item {
    父项

    list {
      item { 子项一 }
      item { 子项二 }
    }
  }
}
```

有序嵌套列表：

```docscript
list(ordered: true) {
  item {
    父项

    list(ordered: true) {
      item { 子项一 }
      item { 子项二 }
    }
  }
}
```

---

## 9. 块级元素

### 标题

```docscript
title { 文档标题 }
h1 { 一级标题 }
h2 { 二级标题 }
h3 { 三级标题 }

heading(level: 2) { 通过 level 指定标题 }
```

`h1` 到 `h6` 是固定级别；`heading` 可以用 `level:` 或第一个位置参数指定级别。

### 段落

```docscript
p { 普通段落 }
paragraph { 同 p }
subtitle { 副标题 }
caption { 说明文字 }
label { 标签文字 }
```

也可以把内联内容作为参数：

```docscript
p("你好，", author)
```

### 引用

```docscript
quote {
  p { 引用内容 }
}
```

### 列表

```docscript
list {
  item { 第一项 }
  item { 第二项 }
}

list(ordered: true) {
  item { 第一项 }
  item { 第二项 }
}
```

`item` 内可以包含文本、段落、图片和其他块。

### 代码块

```docscript
code_block(lang: "rust", `
fn main() {
    println!("hello");
}
`)
```

- 使用反引号包住原始代码。
- 公共缩进会被移除。
- `lang:` 可选。
- 如果每一行都有相同数量的 `\` 前缀，前缀会被去掉。

### 分隔线

```docscript
hr()
divider()
```

### 图片

```docscript
image("glacier.jpg", width: 70%)
image("glacier.jpg", width: 400px)
image("glacier.jpg", height: 200px)
```

- 图片可以独立使用，也可以写在 `p`、`text` 或 `figure` 中。
- `width` 支持百分比、`px`、`pt`、`cm`、`mm`、`in`。
- `height` 可选。
- 未指定宽高时按内容宽度显示。
- 相对路径相对于当前文档文件解析。

### 图注 `figure`

```docscript
figure {
  image("glacier.jpg", width: 70%)
  text {
    strong("Glaciers") form an important part
    of the earth's climate system.
  }
}
```

- 一个 `figure` 必须包含且只能包含一张图片。
- 图注可省略，支持所有内联语法。
- 默认居中；可以设置：

```docscript
@set figure(align: "center", margin_before: 20, margin_after: 20)
```

- HTML 输出 `<figure>` / `<figcaption>`。
- 图注文本会作为图片的 `alt` 文本。

### 表格

表格由 `table`、`row` 和 `cell` 组成：

```docscript
table {
  row {
    cell {
      p { 第一格第一段 }
      p { 第一格第二段 }
    }
    cell {
      p { 第二格 }
    }
  }

  row {
    cell { p { 第三格 } }
    cell {
      list {
        item { 列表项一 }
        item { 列表项二 }
      }
    }
  }
}
```

- `cell` 可以包含任意块级内容，包括多个段落、列表、代码块等。
- 简单文本也可以直接作为参数：`cell("文本")` 或 `cell(text { 文本 })`。
- 单元格内容通常写成 `cell { p { ... } }`，这样可以包含多个段落。
- 建议每一行使用相同数量的 `cell`；不足的格子会补为空单元格。
- 当前列宽等分，不支持 `colspan` / `rowspan`。

样式：

```docscript
@set table(margin_before: 16, margin_after: 16)
@set row(background: "#f9fafb")
@set cell(size: 13, color: #111827, background: "#ffffff")
@set cell.header(background: "#f3f4f6", weight: 700)

table {
  row {
    cell.header { p { 姓名 } }
    cell.header { p { 角色 } }
  }
  row {
    cell { p { 张三 } }
    cell { p { 作者 } }
  }
}
```

- `@set table(...)` 设置整个表格。
- `@set row(...)` 设置行。
- `@set cell(...)` 设置单元格。
- `cell.类名 { ... }` 使用单元格类样式。
- 表格默认带边框和单元格内边距。

---

## 10. 参考书目

### 定义与引用

`@` 后面的名字是自定义参考名，不限于 `reference`：

```docscript
@reference(text { 我是参考书目 })
@paper(text { 张三. DocScript 参考书目. 2026. })

document {
  p { I want to @reference，另见 @paper。 }
}
```

输出时：

- 正文中的 `@reference`、`@paper` 会变成编号标记 `[1]`、`[2]`。
- 编号按第一次被引用的顺序分配。
- 文档末尾自动追加“参考书目”区域，只列出被引用过的条目。
- 参考内容支持内联格式，例如 `strong`、`code`、`link`。

### 参考定义

```docscript
@paper(text {
  strong("DocScript") 语法文档. 2026.
})
```

也可以直接传入字符串：

```docscript
@paper("DocScript 语法文档. 2026.")
```

### 样式

```docscript
@set bibliography(margin_before: 24, margin_after: 12)
```

### 注意

- 参考定义需要在引用前执行。
- `set` 和 `define` 是保留名，不能用作参考名。
- 需要字面量 `@name` 时写成 `\@name`。

---

## 11. 样式系统 `@set`

### 基本形式

```docscript
@set text(font: "Microsoft YaHei", size: 15, color: #24292f)
@set heading(level: 1, size: 32, color: #111827)
@set h2(size: 24, weight: 700, margin_before: 28)
@set page(margin: 2cm)
```

### 作用范围

不带块体的 `@set` 作用于后续内容：

```docscript
@set text(color: #b45309)
p { 这行是棕色 }

p { 这行也是棕色 }
```

带块体的 `@set` 只在块内生效：

```docscript
@set h2(size: 18, color: #b45309) {
  h2 { 只在块内生效的标题 }
  p { 离开块后 h2 恢复原样式。 }
}
```

### 类样式

可以为某个元素定义命名样式类：

```docscript
@set h2.className(@style({
  size: 24;
  color: #111827;
  margin_before: 28;
}))
```

使用时把类名写在元素名后面：

```docscript
h2 { 普通标题 }

h2.className { 使用类样式的标题 }
```

也可以直接使用属性：

```docscript
@set h2.accent(size: 26, color: #b45309)

document {
  h2.accent { 强调标题 }
}
```

规则：

- `@set 元素名.类名(...)` 定义类样式。
- `元素名.类名 { ... }` 使用类样式。
- 类样式会合并到该元素上的普通 `@set` 样式之后，因此类样式优先。
- 类样式只适用于块级元素，例如 `h1` 到 `h6`、`p`、`figure`、自定义元素等。
- 类定义遵循 `@set` 的作用域规则，并且需要在使用前定义。
- 类名不能与内置样式属性同名，例如 `font`、`size`、`color`、`margin`、`style`。
- 当前每个元素只支持一个类名。

### 选择器

| 选择器 | 作用对象 |
| --- | --- |
| `text` | 段落文字 |
| `p`、`paragraph`、`caption`、`label`、`subtitle` | 段落 |
| `title` | 一级标题 |
| `h1` 到 `h6` | 指定级别标题 |
| `heading` | 标题，可配合 `level:` |
| `quote` | 引用 |
| `list` | 列表容器 |
| `item` | 列表项 |
| `code_block` | 代码块 |
| `hr`、`divider` | 分隔线 |
| `figure` | 图片与图注容器 |
| `table` | 表格 |
| `row`、`table_row` | 表格行 |
| `cell`、`table_cell` | 表格单元格 |
| `bibliography`、`references` | 参考书目区域 |
| 自定义名字 | `@define` 元素或带块输出的 `fn` 函数 |

### 样式属性

| 属性 | 说明 |
| --- | --- |
| `font` / `font-family` | 字体名，字符串 |
| `size` / `font-size` | 字号 |
| `weight` / `font-weight` | 字重，如 `400`、`700` |
| `italic` / `font-style` | 布尔值或 `"normal"` 等 |
| `color` | 文字颜色，如 `#333` |
| `background` / `background-color` | 背景色 |
| `leading` / `line-height` | 行高，如 `1.6` 或 `160%` |
| `align` / `text-align` | `"left"`、`"center"`、`"right"`、`"justify"` |
| `indent` / `text-indent` | 首行缩进 |
| `margin` | 同时设置前后间距 |
| `margin_before` / `margin-before` | 段前间距 |
| `margin_after` / `margin-after` | 段后间距 |
| `tracking` / `letter-spacing` | 字距 |
| `style` | 嵌套样式对象 |

属性名中的 `_` 和 `-` 等价。

### `@style` 对象

样式对象可以直接写在 `style:` 中，也可以用 `@style(...)` 包起来：

```docscript
@set h2.style(@style({
  size: 24;
  color: #111827;
  margin_before: 28;
}))
```

### 设置单个属性

```docscript
@set h2.font("Source Han Serif")
@set page.margin(2cm)
```

### 主题与 `@set` 的关系

- 主题提供默认样式。
- `@set` 在主题之上覆盖样式。
- 后定义的 `@set` 会覆盖先定义的同类属性。

### 页面

```docscript
@set page(width: 210mm, height: 297mm, margin: 2cm)
```

`page` 支持：

- `width`
- `height`
- `margin`

---

## 12. 内置函数速查

| 函数 | 说明 |
| --- | --- |
| `document(theme?, title:) { ... }` | 创建文档 |
| `p(...) { ... }` | 段落 |
| `paragraph`、`caption`、`label`、`subtitle` | 段落类 |
| `h1` 到 `h6` | 标题 |
| `heading(level)` | 标题 |
| `title` | 一级标题 |
| `quote { ... }` | 引用 |
| `list(ordered: false) { ... }` | 列表 |
| `item { ... }` | 列表项 |
| `code_block(lang:, 原始代码)` | 代码块 |
| `hr()`、`divider()` | 分隔线 |
| `text`、`span` | 内联内容 |
| `bold`、`strong` | 加粗 |
| `italic`、`em` | 斜体 |
| `code(text)` | 行内代码 |
| `link(text, url)` | 链接 |
| `image(path, width:, height:)` | 图片 |
| `br()`、`line_break()` | 换行 |
| `figure { ... }` | 图片与图注 |
| `table { ... }` | 表格 |
| `row { ... }` | 表格行 |
| `cell { ... }` | 表格单元格 |
| `range(start, end)` | 生成整数列表，右端不包含 |
| `len(value)`、`length(value)` | 长度 |
| `str(...)`、`string(...)` | 转成字符串 |
| `upper(text)` | 大写 |
| `lower(text)` | 小写 |
| `join(list, separator)` | 连接列表 |
| `repeat(count, text)` | 重复文本 |

---

## 13. 渲染与导出

DocScript 编辑器提供实时预览、HTML 导出和 PDF 导出。

- HTML：
  - 页面使用 CSS 排版。
  - 图片会嵌入为 data URI。
  - 参考引用会输出为可点击链接。
  - `figure` 输出为 `<figure>` / `<figcaption>`。
- PDF：
  - 使用字体 shaping 和换行排版。
  - 支持 JPEG、PNG 图片。
  - 自动生成参考书目。
- 预览：
  - 使用 GPUI 渲染，支持文本、列表、图片、图注和参考书目。

PDF 字体可以通过环境变量指定：

```text
DOCSCRIPT_FONT
DOCSCRIPT_BOLD_FONT
DOCSCRIPT_MONO_FONT
```

---

## 14. 完整示例

```docscript
let theme = {
  text: { font: "Microsoft YaHei", size: 15, leading: 1.75, color: "#24292f" },
  h1: { size: 30, weight: 700, color: "#111827" },
  h2: { size: 22, weight: 700 }
}

@set page(margin: 2cm)

@define note(body) {
  quote { {body} }
}
@set note(background: "#f3f4f6", indent: 16, color: "#374151")

let author = "张三"
let topics = ["自由变量", "文本插值", "全局作用域"]
let white = {
  text: { font: "Songti SC", size: 14, leading: 1.6 },
  heading1: { size: 28, weight: 700 },
}

@manual(text { DocScript 语言参考. 2026. })

document(theme, title: "DocScript") {
  h1 { DocScript }

  p {
    这是一个用代码写出来的文档，含 {bold("粗体")}、{code("let x = 1")} 和
    {link("链接", "https://example.com")}。
  }

  p { 作者：{author} }

  h2 { 对象与成员访问 }
  p { white.text.font = {white.text.font} }

  for name, value in white {
    p("样式：", name, "，字号：", value.size)
  }

  h2 { 图片与图注 }

  figure {
    image("glacier.png", width: 70%)
    text {
      strong("Glaciers") form an important part
      of the earth's climate system.
    }
  }

  h2 { 表格 }

  table {
    row {
      cell { p { 姓名 } }
      cell { p { 角色 } }
    }
    row {
      cell {
        p { 张三 }
        p { 第二段说明 }
      }
      cell { p { 作者 } }
    }
  }

  h2 { 参考书目 }
  p { 本文示例参考 @manual。 }

  note(text { 这是一段通过 @define 定义的 note 内容。 })

  for i in range(1, 4) {
    p("第 ", i, " 段由循环生成。")
  }
}
```

---

## 15. 当前限制

- 没有赋值语句，变量只能通过 `let` 定义。
- 没有 `while` 循环。
- 对象迭代顺序当前按字段名排序。
- 参考定义需要在引用前执行。
- PDF 图片格式支持 JPEG 和 PNG。
- 文本块中的嵌套块目前只对 `list` 做行首识别。
- 表格列宽等分，暂不支持 `colspan` / `rowspan`。
- PDF 导出中，表格单元格内的图片目前以占位文本显示；段落、列表和代码块会正常排版。

