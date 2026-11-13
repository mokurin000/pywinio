# Gallery of every widget supported by the PoC.
#
# Run from the repository root:
#   maturin develop --release --offline
#   python .\examples\gallery.py
#
# Set PYWINIO_AUTOCLOSE_MS to auto-close.

import os
import threading

import pywinio as w

app = w.App("pywinio — widget gallery", (900, 640))

root = app.stack(orient="vertical", spacing=10, padding=16)
status = app.label("status", "interact with the widgets")
root.add(status)

tabs = app.tab_view("tabs", selected=0, on_select=lambda: refresh_status())
root.add(tabs, grow=True)


def make_grid(rows: int):
    return app.grid(
        columns="140,1*",
        rows=",".join(["auto"] * rows),
        col_gap=12,
        row_gap=8,
    )


state = {"ticks": 0}


def refresh_status():
    status.text = (
        f"tab={tabs.selected}  edit={edit.text!r}  checked={check.checked}  "
        f"progress={progress.pos}  slider={slider.pos}  "
        f"updown={up.value}  "
        f"combo={combo.selected}  list={lb.selected}  "
        f"radio={radio.checked}  ticks={state['ticks']}"
    )


# --- Basics -------------------------------------------------------------
basics = make_grid(5)
basics_page = app.tab_view_item(tabs, "Basics")
basics_page.set_content(basics)

basics.add(app.label("edit_label", "Edit"), valign="center", row=0, col=0)
edit = app.edit("edit", "type here", on_change=refresh_status)
basics.add(edit, row=0, col=1)

basics.add(app.label("check_label", "CheckBox"), valign="center", row=1, col=0)
check = app.check_box(
    "check",
    "enable something",
    checked=True,
    on_click=refresh_status,
)
basics.add(check, row=1, col=1)

basics.add(app.label("link_label", "LinkLabel"), valign="center", row=2, col=0)


link = app.link_label("link", "click me", uri="https://github.com/compio-rs/winio/")
basics.add(link, row=2, col=1)

basics.add(app.label("progress_label", "Progress"), valign="center", row=3, col=0)
progress = app.progress("progress", pos=35, maximum=100)
basics.add(progress, row=3, col=1)

basics.add(app.label("textbox_label", "TextBox"), valign="center", row=4, col=0)
text_box = app.text_box("textbox", "multi-line\nread-only demo? no, editable")
text_box.tooltip = "You could edit multi-line text here."
basics.add(text_box, row=4, col=1, height=90)

# --- Inputs -------------------------------------------------------------
inputs = make_grid(5)
inputs_page = app.tab_view_item(tabs, "Inputs")
inputs_page.set_content(inputs)

inputs.add(app.label("slider_label", "Slider"), valign="center", row=0, col=0)
slider = app.slider("slider", pos=30, maximum=100, on_change=refresh_status)
inputs.add(slider, row=0, col=1)

inputs.add(app.label("updown_label", "UpDownEdit"), valign="center", row=1, col=0)
up = app.up_down_edit(
    "updown",
    value=5,
    minimum=0,
    maximum=20,
    step=1,
    on_change=refresh_status,
)
inputs.add(up, row=1, col=1)

inputs.add(app.label("combo_label", "ComboBox"), valign="center", row=2, col=0)
combo = app.combo_box(
    "combo",
    ["alpha", "beta", "gamma"],
    selected=0,
    on_select=refresh_status,
    on_change=refresh_status,
)
inputs.add(combo, row=2, col=1)

inputs.add(app.label("radio_label", "RadioButton"), valign="center", row=3, col=0)
radio = app.radio_button(
    "radio",
    "pick me",
    checked=True,
    on_click=refresh_status,
)
inputs.add(radio, row=3, col=1)

inputs.add(app.label("list_label", "ListBox"), valign="center", row=4, col=0)
lb = app.list_box(
    "list",
    [f"item {i}" for i in range(1, 19)],
    selected=[1, 3],
    multiple=True,
    on_select=refresh_status,
)
inputs.add(lb, row=4, col=1, height=140)

# --- Views --------------------------------------------------------------
views = make_grid(1)
views_page = app.tab_view_item(tabs, "Views")
views_page.set_content(views)

views.add(app.label("scroll_label", "ScrollView"), valign="center", row=0, col=0)
scroll = app.scroll_view("scroll", hscroll=False, vscroll=True)
scroll_content = app.stack(orient="vertical", spacing=6, padding=6)
for i in range(20):
    scroll_content.add(app.label(f"scroll_item_{i}", f"scrollable row {i}"))
scroll.set_content(scroll_content)
views.add(scroll, row=0, col=1, height=260)


# --- Timer (headless) ---------------------------------------------------
def on_tick():
    state["ticks"] += 1
    ticks_label.text = f"ticks: {state['ticks']}"


app.timer("tick", interval_ms=250, on_tick=on_tick)

# A small footer that exercises programmatic property updates.
footer = app.stack(orient="horizontal", spacing=8)
root.add(footer, margin=(0, 8, 0, 0))
ticks_label = app.label("ticks", "ticks: 0")
footer.add(ticks_label, valign="center")


def bump_progress():
    progress.pos = min(progress.pos + 10, progress.maximum)
    refresh_status()


def toggle_check():
    check.checked = not check.checked
    refresh_status()


footer.add(app.button("bump", "Progress +10", on_click=bump_progress))
footer.add(app.button("toggle", "Toggle check", on_click=toggle_check))
footer.add(app.button("close", "Close", on_click=app.quit))

app.root = root

autoclose = os.environ.get("PYWINIO_AUTOCLOSE_MS")
if autoclose:
    timer = threading.Timer(int(autoclose) / 1000.0, app.quit)
    timer.daemon = True
    timer.start()

app.run()
print(f"gallery closed: {status.text}", flush=True)
