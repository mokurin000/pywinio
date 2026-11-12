"""pywinio — experimental Python bindings for winio.

Python declares the widget tree and the layout; Rust owns the winio component
lifecycle (``init``/``start``/``update``/``render``) and dispatches widget
events back to Python callbacks.

Implemented widgets: ``Button``, ``Label``, ``Edit``, ``CheckBox``,
``LinkLabel``, ``Progress``, ``TextBox``, ``Slider`` and ``UpDownEdit``, plus
``StackPanel``/``Grid`` layout containers. WebView and the compio compatibility
layer are intentionally out of scope.
"""

from ._pywinio import (
    App,
    Button,
    CheckBox,
    Edit,
    Grid,
    Label,
    LinkLabel,
    Progress,
    Slider,
    StackPanel,
    TextBox,
    UpDownEdit,
    __version__,
)

__all__ = [
    "App",
    "Button",
    "CheckBox",
    "Edit",
    "Grid",
    "Label",
    "LinkLabel",
    "Progress",
    "Slider",
    "StackPanel",
    "TextBox",
    "UpDownEdit",
    "__version__",
]
