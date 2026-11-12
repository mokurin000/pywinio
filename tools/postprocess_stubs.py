"""Post-process the maturin-generated stub.

pyo3 keeps annotations from ``#[pyo3(signature = (...))]`` as string literals,
so maturin emits ``orient: "Literal['vertical', 'horizontal']"`` and does not
import ``Literal``. Rewrite those annotations to real ``Literal[...]`` types and
make sure ``typing.Literal`` is imported.

Usage:
    python tools/postprocess_stubs.py [path/to/_pywinio.pyi]
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

DEFAULT = Path("python/pywinio/_pywinio.pyi")


def ensure_literal_import(text: str) -> str:
    marker = "from typing import "
    start = text.find(marker)
    if start < 0:
        return "from typing import Literal\n\n" + text
    end = text.find("\n", start)
    line = text[start:end]
    if re.search(r"\bLiteral\b", line):
        return text
    new_line = line.replace(marker, marker + "Literal, ", 1)
    return text[:start] + new_line + text[end:]


def main() -> int:
    path = Path(sys.argv[1]) if len(sys.argv) > 1 else DEFAULT
    if not path.is_file():
        print(f"stub not found: {path}", file=sys.stderr)
        return 1

    text = path.read_text(encoding="utf-8")
    text = ensure_literal_import(text)
    path.write_text(text, encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
