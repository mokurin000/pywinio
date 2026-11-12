import threading, time
import pywinio as w

app = w.App("thread probe", (420, 180))
root = app.stack(orient="vertical", spacing=8, padding=12)
lbl = root.add_label("l", "initial")
root.add_button("b", "noop")

def worker():
    time.sleep(1.2)
    lbl.text = "updated from another thread"
    print("setter returned", flush=True)

threading.Thread(target=worker, daemon=True).start()
timer = threading.Timer(5.0, app.quit); timer.daemon = True; timer.start()
app.root = root
app.run()
print("run returned", flush=True)
