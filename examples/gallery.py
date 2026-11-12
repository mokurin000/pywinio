# Gallery of every widget supported by the PoC.
#
# Run from the repository root:
#   .\build.ps1 -Offline
#   $env:PYTHONPATH = "python"; python .\examples\gallery.py
#
# Set PYWINIO_AUTOCLOSE_MS to auto-close.

import os
import threading

import pywinio as w

app = w.App("pywinio — widget gallery", (900, 640))

root = app.stack(orient="vertical", spacing=10, padding=16)
status = app.label("status", "interact with the widgets")
root.add(status)

grid = app.grid(
    columns="140,1*",
    rows=",".join(["auto"] * 11),
    col_gap=12,
    row_gap=8,
)
root.add(grid, grow=True)

# --- Edit ---------------------------------------------------------------
grid.add(app.label("edit_label", "Edit"), valign="center", row=0, col=0)

state = {"links": 0, "ticks": 0}


def refresh_status():
    status.text = (
        f"edit={edit.text!r}  checked={check.checked}  "
        f"progress={progress.pos}  slider={slider.pos}  "
        f"updown={up.value}  links={state['links']}  "
        f"combo={combo.selected}  list={lb.selected}  "
        f"radio={radio.checked}  ticks={state['ticks']}"
    )


edit = app.edit("edit", "type here", on_change=refresh_status)
grid.add(edit, row=0, col=1)

# --- CheckBox -----------------------------------------------------------
grid.add(app.label("check_label", "CheckBox"), valign="center", row=1, col=0)
check = app.check_box(
    "check",
    "enable something",
    checked=True,
    on_click=refresh_status,
)
grid.add(check, row=1, col=1)

# --- LinkLabel ----------------------------------------------------------
grid.add(app.label("link_label", "LinkLabel"), valign="center", row=2, col=0)


def on_link():
    state["links"] += 1
    refresh_status()


link = app.link_label("link", "click me (no uri)", on_click=on_link)
grid.add(link, row=2, col=1)

# --- Progress -----------------------------------------------------------
grid.add(app.label("progress_label", "Progress"), valign="center", row=3, col=0)
progress = app.progress("progress", pos=35, maximum=100)
grid.add(progress, row=3, col=1)

# --- TextBox ------------------------------------------------------------
grid.add(app.label("textbox_label", "TextBox"), valign="center", row=4, col=0)
text_box = app.text_box("textbox", "multi-line\nread-only demo? no, editable")
grid.add(text_box, row=4, col=1, height=90)

# --- Slider -------------------------------------------------------------
grid.add(app.label("slider_label", "Slider"), valign="center", row=5, col=0)
slider = app.slider("slider", pos=30, maximum=100, on_change=refresh_status)
grid.add(slider, row=5, col=1)

# --- UpDownEdit ---------------------------------------------------------
grid.add(app.label("updown_label", "UpDownEdit"), valign="center", row=6, col=0)
up = app.up_down_edit(
    "updown",
    value=5,
    minimum=0,
    maximum=20,
    step=1,
    on_change=refresh_status,
)
grid.add(up, row=6, col=1)

# --- ComboBox -----------------------------------------------------------
grid.add(app.label("combo_label", "ComboBox"), valign="center", row=7, col=0)
combo = app.combo_box(
    "combo",
    ["alpha", "beta", "gamma"],
    selected=0,
    on_select=refresh_status,
    on_change=refresh_status,
)
grid.add(combo, row=7, col=1)

# --- RadioButton --------------------------------------------------------
grid.add(app.label("radio_label", "RadioButton"), valign="center", row=8, col=0)
radio = app.radio_button(
    "radio",
    "pick me",
    checked=True,
    on_click=refresh_status,
)
grid.add(radio, row=8, col=1)

# --- ListBox ------------------------------------------------------------
grid.add(app.label("list_label", "ListBox"), valign="center", row=9, col=0)
lb = app.list_box(
    "list",
    [f"item {i}" for i in range(1, 9)],
    selected=[1, 3],
    multiple=True,
    on_select=refresh_status,
)
grid.add(lb, row=9, col=1, height=120)

# --- ScrollView ---------------------------------------------------------
grid.add(app.label("scroll_label", "ScrollView"), valign="center", row=10, col=0)
scroll = app.scroll_view("scroll", hscroll=False, vscroll=True)
scroll_content = app.stack(orient="vertical", spacing=6, padding=6)
for i in range(12):
    scroll_content.add(app.label(f"scroll_item_{i}", f"scrollable row {i}"))
scroll.set_content(scroll_content)
grid.add(scroll, row=10, col=1, height=140)


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
