pub const SAMPLE: &str = r##"// DocScript：结构是函数调用，文本写在文本块里
// 没有 # 标题、没有 *粗体*、没有 - 列表
// 图片：image("glacier.jpg", width: 70%)，可写在 p 内；figure 可添加图注
// 参考：@name(text { ... }) 定义，正文中用 @name 引用

let theme = {
  text: {
    font: "Microsoft YaHei",
    size: 15,
    leading: 1.8,
    color: "#24292f"
  },
  h1: { size: 34, weight: 700, color: "#111827", margin_after: 20 },
  h2: { size: 24, weight: 700, color: "#111827", margin_before: 28 },
  quote: { color: "#57606a", indent: 20 },
  code: { font: "Cascadia Mono", size: 13, color: "#b31d28" },
  code_block: { font: "Cascadia Mono", size: 13, leading: 1.6 }
}

// 作用域样式：@set 作用于后续内容；带 { } 时只在该块内生效
@set text(font: "Microsoft YaHei", size: 15, color: #24292f)
@set heading(level: 1, size: 34, color: #111827)
@set h2.style(@style({
  size: 24;
  color: #111827;
  margin_before: 28;
}))
@set h2.accent(@style({
  color: #b45309;
  margin_before: 32;
}))
@set cell.header(background: "#f3f4f6", weight: 700)
@set page(margin: 2cm)

// @define 声明自定义语义元素
@define note(body) {
  quote { {body} }
}
@set note(background: #f3f4f6, indent: 16, color: #374151)

// 顶层 let 是全局自由变量，函数与文本插值都能访问
let author = "张三"
let topics = ["自由变量", "文本插值", "全局作用域"]

let white = {
  text: { font: "Songti SC", size: 14, leading: 1.6 },
  heading1: { size: 28, weight: 700 },
}

fn byline() {
  p { 作者：{author} }
}

// fn 定义的函数也可以像自定义元素一样用 @set 设置容器样式
@set byline(background: "#eef2ff", color: "#3730a3", indent: 14)

// @name(...) 定义参考书目，正文中用 @name 引用
@manual(text { DocScript 语言参考. 2026. })

document(theme, title: "DocScript") {
  h1 { DocScript }

  p {
    这是一个用代码写出来的文档。结构由显式命名的函数调用组成：
    h1、p、list、item、quote。文本不需要转义 Markdown 符号，
    因为这里根本没有 Markdown 符号。
  }

  h2 { 文本与表达式 }

  p {
    文本块里可以直接插入表达式，例如变量、函数调用和计算：
    {bold("粗体")}、{code("let x = 1")}、{link("一个链接", "https://example.com")}。
    也可以写 {2 + 3} 这样的表达式。
  }

  p {
    中英文混排会自动处理空格。连续的换行会合并成同一段，
    但中文之间不会插入多余的空格。
  }

  h2 { 列表 }

  list {
    item { 每一项都是显式命名的 item }
    item { 列表本身也是函数：list、ordered: true }
    item {
      文本块可以嵌套行内表达式，例如 {strong("加粗")}
      list {
        item { 这是 item 内部的嵌套列表 }
        item { 嵌套列表仍然是真正的块结构 }
      }
    }
  }

  list(ordered: true) {
    item { 第一项 }
    item { 第二项 }
    item { 第三项 }
  }

  h2 { 作用域样式 }

  @set h2(size: 20, color: #b45309) {
    h2 { 局部作用域标题 }
    p { 这个块结束后，后面的 h2 会恢复原来的样式。 }
  }

  h2.accent { 类样式标题 }

  p { 这个标题使用了 h2.accent 定义的类样式。 }

  h2 { 代码能力 }

  for i in range(1, 4) {
    p("第 ", i, " 段由 for 循环生成。")
  }

  if true {
    p { 条件语句同样可用。 }
  }

  note(text {
    引用也可以通过 @define 自定义语义元素生成。
  })

  h2 { 自由变量与文本插值 }

  byline()

  for topic in topics {
    p { 主题：{topic}。 }
  }

  h2 { 对象与成员访问 }

  p { white.text.font = {white.text.font} }

  for name, value in white {
    p("样式：", name, "，字号：", value.size)
  }

  h2 { 表格 }

  table {
    row {
      cell.header { p { 姓名 } }
      cell.header { p { 角色 } }
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

  h2 { 代码块 }

  code_block(lang: "docscript", `
fn hello(name) {
  p("你好，", name, "！")
}

hello("世界")
`)

  p {
    所有内容最终会被编译成文档模型，然后渲染到左侧预览、
    导出为 HTML 或 PDF。
  }
}
"##;
