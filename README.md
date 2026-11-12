# pywinio

[winio](https://docs.rs/winio) 的 Python 绑定原型（pyo3）。

当前是 **PoC v0.1**：Python 声明组件树与布局，Rust 侧全自动接管 winio 的
`init` / `start` / `update` / `render` 生命周期，原生事件通过统一通道回调到 Python。
按计划不包含 WebView 与 `compio-compat`。

![widget gallery](docs/images/gallery.png)

## 已实现

- 组件：`Button`、`Label`、`Edit`、`CheckBox`、`LinkLabel`、`Progress`、`TextBox`、`Slider`、`UpDownEdit`、`ComboBox`、`RadioButton`、`ListBox`、`ScrollView`、`Timer`（headless）
- 布局：`StackPanel`（横/竖、spacing、padding、grow、对齐）、`Grid`（固定/auto/权重轨道、跨行跨列、gap），可任意嵌套
- 运行时：`label.text = ...`、`slider.pos = ...` 等属性修改会投递到 UI 线程并触发重排
- 回调：`on_click` / `on_change` / `on_select` / `on_tick` 注册，自动接到 winio 的 `start`/`update`
- 关闭：标题栏关闭、`Close` 按钮回调 `app.quit()`、跨线程 `app.quit()` 均可用

## 快速开始（Windows）

环境：Rust nightly、Python 3.13、Windows 10/11（win32 后端）。

```powershell
# 如果 PowerShell 禁止执行脚本：
# Set-ExecutionPolicy -Scope Process Bypass

.\build.ps1 -Offline
$env:PYTHONPATH = "$PWD\python"
python .\examples\counter.py
```

自动关闭（用于验证/CI）：

```powershell
$env:PYWINIO_AUTOCLOSE_MS = "1500"; python .\examples\counter.py
```

> 本机 cargo/git 的 schannel TLS 栈无法访问 crates.io，但依赖已预缓存，所以这里用
> `-Offline` 构建；正常联网环境直接 `.\build.ps1` 即可。

## 构建 CPython wheel

需要 [maturin](https://maturin.rs/)（`python -m pip install maturin`）：

```powershell
.\build-wheel.ps1 -Offline
# 等价于: maturin generate-stubs --out python --interpreter python --offline
#         maturin build --release --offline

# 安装构建出的 wheel
python -m pip install --force-reinstall target\wheels\pywinio-*.whl
```

wheel 标签跟随当前解释器（本机为 `cp313-cp313-win_amd64`），尚未启用 abi3。
在虚拟环境中开发时，也可以用 `maturin develop` 直接安装到该环境。

## 类型存根（`.pyi`）

`#[pymethods]` 的签名由 maturin 的 pyo3 内省生成：

```powershell
maturin generate-stubs --out python --interpreter python
```

`build-wheel.ps1` 会先执行该命令刷新 `python/pywinio/_pywinio.pyi`，再构建 wheel；
wheel 内同时包含 `_pywinio.pyi` 与 `py.typed`。pyright 对生成结果的检查为 0 errors。

> 需要 `pyo3` 的 `experimental-inspect` feature（已在 `Cargo.toml` 打开），
> 且扩展模块必须是 `#[pymodule] mod` 内联模块（当前已满足）。
> 由于 stub 已提交，`maturin build --generate-stubs` 会报
> “overwrote an existing tracked file”；请用 `build-wheel.ps1`，或先
> `generate-stubs` 再普通 `maturin build`。

## API 速览

```python
import pywinio as w

app = w.App("Counter", (640, 480))

root = app.stack(orient="vertical", spacing=8, padding=16)
counter = app.label("counter", "0 clicks")
root.add(counter)

def inc():
    counter.text = "1 clicks"   # 运行时改属性 -> 投递命令 -> 重排

root.add(app.button("inc", "+1", on_click=inc))

app.root = root
app.run()      # 阻塞；回调在 UI 线程执行
```

更完整的嵌套/Grid 示例见 [examples/counter.py](examples/counter.py)，
全组件示例见 [examples/gallery.py](examples/gallery.py)。

所有组件统一由 `App.xxx()` 创建，再用 `container.add(widget, grow=..., margin=..., halign=...)`
一次性加入布局；容器本身不再提供 `add_button` / `add_label` / `add_stack` / `add_grid`。

新增组件通过 `App` 工厂创建，再用通用 `add` 放入布局：

```python
combo = app.combo_box("combo", ["alpha", "beta"], selected=0, on_select=refresh)
listbox = app.list_box("list", ["one", "two"], selected=[0], multiple=True, on_select=refresh)
radio = app.radio_button("radio", "choice", checked=True, on_click=refresh)

scroll = app.scroll_view("scroll", hscroll=False, vscroll=True)
content = app.stack(orient="vertical")
content.add(app.label("row", "scrollable content"))
scroll.set_content(content)          # 必须在 run() 前
grid.add(scroll, row=0, col=1, height=140)

app.timer("tick", interval_ms=250, on_tick=on_tick)   # 不参与布局
```

## 设计文档

架构、消息流、自动注册原理、布局算法、取舍与路线图：
[docs/design.md](docs/design.md)。

## 测试

```powershell
# 布局引擎单测（7 个）：用 --no-default-features 让测试二进制链接 libpython，
# 而不是按 extension-module 方式构建
cargo test --offline --no-default-features
```

## 仓库结构

```
pyproject.toml  maturin / PEP 621 元数据（mixed Rust/Python 布局）
build-wheel.ps1 构建 CPython wheel
src/lib.rs      pyo3 边界：App / 容器 / 14 种 widget 与 Timer 的 pyclass
src/model.rs    声明期数据模型、节点注册表、运行时 sender 表
src/widget.rs   AnyWidget：所有组件的统一 Component 实现（自动 start/update）
src/root.rs     PyRoot：窗口、动态子组件 fan-in、布局、事件分发
src/layout.rs   measure/arrange 两遍布局引擎（含单测）
python/         Python 包（加载构建出的 _pywinio.pyd）、_pywinio.pyi、py.typed
examples/       counter.py、gallery.py
docs/           设计草案与截图
```
