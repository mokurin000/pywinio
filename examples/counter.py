# pywinio PoC: Button + Label, nested StackPanel/Grid layout.
#
# Run from the repository root:
#
#   .\build.ps1
#   $env:PYTHONPATH = "python"; python .\examples\counter.py
#
# Set PYWINIO_AUTOCLOSE_MS to close the window automatically (useful for CI).

import os
import threading

import pywinio as w

app = w.App("pywinio PoC — Button + Label", (720, 480))

# Root: a vertical stack. All children are pre-registered before app.run().
root = app.stack(orient="vertical", spacing=12, padding=16)

header = root.add_label("header", "pywinio — declarative Python UI on winio")

# A grid with two weighted columns; the left column holds labels, the right
# column holds a horizontal stack of buttons. This exercises nesting.
grid = app.grid(columns="1*,1*", rows="auto", col_gap=16, row_gap=8)
root.add(grid, grow=True)

left = grid.add_stack(orient="vertical", spacing=6, row=0, col=0)
counter = left.add_label("counter", "0 clicks")
left.add_label("hint", "The labels are updated from the button callback.")

right = grid.add_stack(orient="horizontal", spacing=8, row=0, col=1)

clicks = 0


def add(delta):
    global clicks
    clicks += delta
    counter.text = f"{clicks} clicks"
    header.text = f"pywinio — {clicks} click(s) so far"


def reset():
    global clicks
    clicks = 0
    counter.text = "0 clicks"
    header.text = "pywinio — declarative Python UI on winio"


inc = right.add_button("inc", "+1", on_click=lambda: add(1))
dec = right.add_button("dec", "−1", on_click=lambda: add(-1))
right.add_button("reset", "Reset", on_click=reset)

footer = app.stack(orient="horizontal", spacing=8)
root.add(footer, margin=(0, 8, 0, 0))
footer.add_button("close", "Close", on_click=app.quit)

app.root = root

autoclose = os.environ.get("PYWINIO_AUTOCLOSE_MS")
if autoclose:
    timer = threading.Timer(int(autoclose) / 1000.0, app.quit)
    timer.daemon = True
    timer.start()

app.run()
print(f"window closed, counter = {clicks}")
