from __future__ import annotations

from sqlalchemy.orm import DeclarativeBase, Mapped


class Base(DeclarativeBase):
    some_mapping: Mapped[list[Bar]] | None = None  # Should not trigger F821 (resolveable forward reference)
    simplified: list[Bar] | None = None  # Should not trigger F821 (resolveable forward reference)


class Bar:
    pass
