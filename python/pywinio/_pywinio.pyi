from typing import Any, Final, SupportsFloat, SupportsIndex, final

__all__ = ["__version__", "App", "Button", "CheckBox", "Edit", "Grid", "Label", "LinkLabel", "Progress", "Slider", "StackPanel", "TextBox", "UpDownEdit"]

__version__: Final[str]

@final
class App:
    """
    The application object.

    Create it on the Python main thread, describe the UI, then call
    [`run`](PyApp::run). `run` releases the GIL and blocks until the window is
    closed (or [`quit`](PyApp::quit) is called).
    """
    def __new__(cls, /, title: str, size: tuple[SupportsFloat | SupportsIndex, SupportsFloat | SupportsIndex] = ...) -> App: ...
    def check_box(self, /, id: str, text: str = "", checked: bool = False, *, on_click: Any | None = None) -> CheckBox:
        """
        Create a check box.
        """
    def edit(self, /, id: str, text: str = "", *, on_change: Any | None = None) -> Edit:
        """
        Create a single-line text input.

        Add it to a container with `container.add(edit, ...)`.
        """
    def grid(self, /, columns: str = "", rows: str = "", col_gap: SupportsFloat | SupportsIndex = 0.0, row_gap: SupportsFloat | SupportsIndex = 0.0, padding: SupportsFloat | SupportsIndex = 0.0) -> Grid:
        """
        Create a grid container.

        `columns` and `rows` use winio's track notation: `"1*,2*"` for weighted
        tracks, `"120"` for a fixed track and `"auto"` for content sizing.
        """
    def link_label(self, /, id: str, text: str = "", uri: str = "", *, on_click: Any | None = None) -> LinkLabel:
        """
        Create a hyperlink-style label.

        If `uri` is non-empty, clicking opens the URI and no `on_click`
        callback is invoked (winio behavior).
        """
    def progress(self, /, id: str, pos: SupportsIndex = 0, minimum: SupportsIndex = 0, maximum: SupportsIndex = 100, indeterminate: bool = False) -> Progress:
        """
        Create a progress bar.
        """
    def quit(self, /) -> None:
        """
        Ask the root component to close the window.

        Safe to call from another Python thread while `run()` is blocking.
        """
    @property
    def root(self, /) -> Any | None:
        """
        The root layout container. Must be set before `run()`.
        """
    @root.setter
    def root(self, /, node: Any) -> None: ...
    def run(self, /) -> None:
        """
        Run the window loop until the window is closed.

        This blocks the calling thread. It must be called from the same thread
        that created the `App`.
        """
    @property
    def running(self, /) -> bool:
        """
        Whether the window loop is currently running.
        """
    def slider(self, /, id: str, pos: SupportsIndex = 0, minimum: SupportsIndex = 0, maximum: SupportsIndex = 100, *, on_change: Any | None = None) -> Slider:
        """
        Create a slider.
        """
    def stack(self, /, orient: str = "vertical", spacing: SupportsFloat | SupportsIndex = 0.0, padding: SupportsFloat | SupportsIndex = 0.0) -> StackPanel:
        """
        Create a vertical or horizontal stack container.
        """
    def text_box(self, /, id: str, text: str = "") -> TextBox:
        """
        Create a multi-line text input.
        """
    @property
    def title(self, /) -> str: ...
    def up_down_edit(self, /, id: str, value: SupportsIndex = 0, minimum: SupportsIndex = 0, maximum: SupportsIndex = 100, step: SupportsIndex = 1, *, on_change: Any | None = None) -> UpDownEdit:
        """
        Create an integer input with up/down buttons.
        """

@final
class Button:
    """
    A clickable button.
    """
    def __repr__(self, /) -> str: ...
    @property
    def enabled(self, /) -> bool: ...
    @enabled.setter
    def enabled(self, /, enabled: bool) -> None: ...
    @property
    def id(self, /) -> str: ...
    @property
    def on_click(self, /) -> Any | None:
        """
        The click callback. Can be replaced while the window is running.
        """
    @on_click.setter
    def on_click(self, /, callback: Any | None) -> None: ...
    @property
    def text(self, /) -> str: ...
    @text.setter
    def text(self, /, text: str) -> None: ...
    @property
    def visible(self, /) -> bool: ...
    @visible.setter
    def visible(self, /, visible: bool) -> None: ...

@final
class CheckBox:
    """
    A check box.
    """
    def __repr__(self, /) -> str: ...
    @property
    def checked(self, /) -> bool: ...
    @checked.setter
    def checked(self, /, checked: bool) -> None: ...
    @property
    def enabled(self, /) -> bool: ...
    @enabled.setter
    def enabled(self, /, enabled: bool) -> None: ...
    @property
    def id(self, /) -> str: ...
    @property
    def on_click(self, /) -> Any | None: ...
    @on_click.setter
    def on_click(self, /, callback: Any | None) -> None: ...
    @property
    def text(self, /) -> str: ...
    @text.setter
    def text(self, /, text: str) -> None: ...
    @property
    def visible(self, /) -> bool: ...
    @visible.setter
    def visible(self, /, visible: bool) -> None: ...

@final
class Edit:
    """
    A single-line text input.
    """
    def __repr__(self, /) -> str: ...
    @property
    def enabled(self, /) -> bool: ...
    @enabled.setter
    def enabled(self, /, enabled: bool) -> None: ...
    @property
    def id(self, /) -> str: ...
    @property
    def on_change(self, /) -> Any | None: ...
    @on_change.setter
    def on_change(self, /, callback: Any | None) -> None: ...
    @property
    def text(self, /) -> str: ...
    @text.setter
    def text(self, /, text: str) -> None: ...
    @property
    def visible(self, /) -> bool: ...
    @visible.setter
    def visible(self, /, visible: bool) -> None: ...

@final
class Grid:
    """
    A grid layout container.
    """
    def add(self, /, child: Any, *, row: SupportsIndex = 0, col: SupportsIndex = 0, row_span: SupportsIndex = 1, col_span: SupportsIndex = 1, margin: tuple[SupportsFloat | SupportsIndex, SupportsFloat | SupportsIndex, SupportsFloat | SupportsIndex, SupportsFloat | SupportsIndex] | None = None, halign: str = "stretch", valign: str = "stretch", width: SupportsFloat | SupportsIndex | None = None, height: SupportsFloat | SupportsIndex | None = None) -> None:
        """
        Add a widget or a nested container at a grid position.
        """
    def add_button(self, /, id: str, text: str, *, on_click: Any | None = None, row: SupportsIndex = 0, col: SupportsIndex = 0, row_span: SupportsIndex = 1, col_span: SupportsIndex = 1) -> Button:
        """
        Create a button and append it at a grid position.
        """
    def add_label(self, /, id: str, text: str, *, row: SupportsIndex = 0, col: SupportsIndex = 0, row_span: SupportsIndex = 1, col_span: SupportsIndex = 1) -> Label:
        """
        Create a label and append it at a grid position.
        """
    def add_stack(self, /, orient: str = "vertical", spacing: SupportsFloat | SupportsIndex = 0.0, padding: SupportsFloat | SupportsIndex = 0.0, *, row: SupportsIndex = 0, col: SupportsIndex = 0, row_span: SupportsIndex = 1, col_span: SupportsIndex = 1) -> StackPanel:
        """
        Create a nested stack and append it at a grid position.
        """
    @property
    def columns(self, /) -> int: ...
    @property
    def rows(self, /) -> int: ...

@final
class Label:
    """
    A read-only text widget.
    """
    def __repr__(self, /) -> str: ...
    @property
    def id(self, /) -> str: ...
    @property
    def text(self, /) -> str: ...
    @text.setter
    def text(self, /, text: str) -> None: ...
    @property
    def visible(self, /) -> bool: ...
    @visible.setter
    def visible(self, /, visible: bool) -> None: ...

@final
class LinkLabel:
    """
    A hyperlink-style label.
    """
    def __repr__(self, /) -> str: ...
    @property
    def enabled(self, /) -> bool: ...
    @enabled.setter
    def enabled(self, /, enabled: bool) -> None: ...
    @property
    def id(self, /) -> str: ...
    @property
    def on_click(self, /) -> Any | None: ...
    @on_click.setter
    def on_click(self, /, callback: Any | None) -> None: ...
    @property
    def text(self, /) -> str: ...
    @text.setter
    def text(self, /, text: str) -> None: ...
    @property
    def uri(self, /) -> str: ...
    @uri.setter
    def uri(self, /, uri: str) -> None: ...
    @property
    def visible(self, /) -> bool: ...
    @visible.setter
    def visible(self, /, visible: bool) -> None: ...

@final
class Progress:
    """
    A progress bar.
    """
    def __repr__(self, /) -> str: ...
    @property
    def enabled(self, /) -> bool: ...
    @enabled.setter
    def enabled(self, /, enabled: bool) -> None: ...
    @property
    def id(self, /) -> str: ...
    @property
    def indeterminate(self, /) -> bool: ...
    @indeterminate.setter
    def indeterminate(self, /, indeterminate: bool) -> None: ...
    @property
    def maximum(self, /) -> int: ...
    @maximum.setter
    def maximum(self, /, maximum: SupportsIndex) -> None: ...
    @property
    def minimum(self, /) -> int: ...
    @minimum.setter
    def minimum(self, /, minimum: SupportsIndex) -> None: ...
    @property
    def pos(self, /) -> int: ...
    @pos.setter
    def pos(self, /, pos: SupportsIndex) -> None: ...
    @property
    def visible(self, /) -> bool: ...
    @visible.setter
    def visible(self, /, visible: bool) -> None: ...

@final
class Slider:
    """
    A slider.
    """
    def __repr__(self, /) -> str: ...
    @property
    def enabled(self, /) -> bool: ...
    @enabled.setter
    def enabled(self, /, enabled: bool) -> None: ...
    @property
    def id(self, /) -> str: ...
    @property
    def maximum(self, /) -> int: ...
    @maximum.setter
    def maximum(self, /, maximum: SupportsIndex) -> None: ...
    @property
    def minimum(self, /) -> int: ...
    @minimum.setter
    def minimum(self, /, minimum: SupportsIndex) -> None: ...
    @property
    def on_change(self, /) -> Any | None: ...
    @on_change.setter
    def on_change(self, /, callback: Any | None) -> None: ...
    @property
    def pos(self, /) -> int: ...
    @pos.setter
    def pos(self, /, pos: SupportsIndex) -> None: ...
    @property
    def visible(self, /) -> bool: ...
    @visible.setter
    def visible(self, /, visible: bool) -> None: ...

@final
class StackPanel:
    """
    A stacked layout container.
    """
    def add(self, /, child: Any, *, grow: bool = False, margin: tuple[SupportsFloat | SupportsIndex, SupportsFloat | SupportsIndex, SupportsFloat | SupportsIndex, SupportsFloat | SupportsIndex] | None = None, halign: str = "stretch", valign: str = "stretch", width: SupportsFloat | SupportsIndex | None = None, height: SupportsFloat | SupportsIndex | None = None) -> None:
        """
        Add a widget or a nested container.
        """
    def add_button(self, /, id: str, text: str, *, on_click: Any | None = None) -> Button:
        """
        Create a button and append it.
        """
    def add_grid(self, /, columns: str = "", rows: str = "", col_gap: SupportsFloat | SupportsIndex = 0.0, row_gap: SupportsFloat | SupportsIndex = 0.0, padding: SupportsFloat | SupportsIndex = 0.0) -> Grid:
        """
        Create a nested grid and append it.
        """
    def add_label(self, /, id: str, text: str) -> Label:
        """
        Create a label and append it.
        """
    def add_stack(self, /, orient: str = "vertical", spacing: SupportsFloat | SupportsIndex = 0.0, padding: SupportsFloat | SupportsIndex = 0.0) -> StackPanel:
        """
        Create a nested stack and append it.
        """
    @property
    def orient(self, /) -> str: ...

@final
class TextBox:
    """
    A multi-line text input.
    """
    def __repr__(self, /) -> str: ...
    @property
    def enabled(self, /) -> bool: ...
    @enabled.setter
    def enabled(self, /, enabled: bool) -> None: ...
    @property
    def id(self, /) -> str: ...
    @property
    def text(self, /) -> str: ...
    @text.setter
    def text(self, /, text: str) -> None: ...
    @property
    def visible(self, /) -> bool: ...
    @visible.setter
    def visible(self, /, visible: bool) -> None: ...

@final
class UpDownEdit:
    """
    An integer input with up/down buttons.
    """
    def __repr__(self, /) -> str: ...
    @property
    def enabled(self, /) -> bool: ...
    @enabled.setter
    def enabled(self, /, enabled: bool) -> None: ...
    @property
    def id(self, /) -> str: ...
    @property
    def maximum(self, /) -> int: ...
    @maximum.setter
    def maximum(self, /, maximum: SupportsIndex) -> None: ...
    @property
    def minimum(self, /) -> int: ...
    @minimum.setter
    def minimum(self, /, minimum: SupportsIndex) -> None: ...
    @property
    def on_change(self, /) -> Any | None: ...
    @on_change.setter
    def on_change(self, /, callback: Any | None) -> None: ...
    @property
    def step(self, /) -> int: ...
    @step.setter
    def step(self, /, step: SupportsIndex) -> None: ...
    @property
    def value(self, /) -> int: ...
    @value.setter
    def value(self, /, value: SupportsIndex) -> None: ...
    @property
    def visible(self, /) -> bool: ...
    @visible.setter
    def visible(self, /, visible: bool) -> None: ...
