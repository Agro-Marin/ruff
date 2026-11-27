# Lint should account for deferred annotations
# See https://github.com/astral-sh/ruff/issues/15857

from __future__ import annotations

import typing


class Example():
    options: ClassVar[dict[str, str]] = {}


if typing.TYPE_CHECKING:
    from typing import ClassVar
