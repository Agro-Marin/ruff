from typing import Generic
from typing_extensions import TypeVar

T = TypeVar("T", default=str)


class DefaultTypeVar(Generic[T]):
    var: T


class KeywordArguments(Generic[T], metaclass=type):
    var: T
