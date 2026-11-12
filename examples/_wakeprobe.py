import threading, time
import pywinio as w

app = w.App("wake probe", (360, 160))
root = app.stack(orient="vertical", spacing=8, padding=12)
root.add_label("l", "idle")
app.root = root

t0 = time.monotonic()
timer = threading.Timer(2.5, lambda: (print(f"timer fire at {time.monotonic()-t0:.2f}s", flush=True), app.quit()))
timer.daemon = True
timer.start()
app.run()
print(f"run returned at {time.monotonic()-t0:.2f}s", flush=True)
