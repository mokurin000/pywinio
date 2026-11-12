"""pywinio — experimental Python bindings for winio.

This is the first proof of concept: Python declares the widget tree and the
layout, Rust owns the winio component lifecycle (``init``/``start``/``update``/
``render``) and dispatches widget events back to Python callbacks.

Only ``Button`` and ``Label``, plus ``StackPanel``/``Grid`` layout containers,
are implemented in this milestone. WebView and the compio compatibility layer
are intentionally out of scope.
"""

from ._pywinio import (
    App,
    Button,
    Grid,
    Label,
    StackPanel,
    __version__,
)

__all__ = [
    "App",
    "Button",
    "Grid",
    "Label",
    "StackPanel",
    "__version__",
]
