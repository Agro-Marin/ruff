from dataclasses import dataclass
from typing import ClassVar, Optional


@dataclass(frozen=True)
class EmptyCell:
    _singleton: ClassVar[Optional["EmptyCell"]] = None
    # the behavior of _singleton above should match a non-ClassVar
    _doubleton: "EmptyCell"
