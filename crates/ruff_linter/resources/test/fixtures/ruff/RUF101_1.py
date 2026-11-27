# ruff: noqa: TCH002

from __future__ import annotations

import local_module


def func(sized: local_module.Container) -> int:
    return len(sized)
