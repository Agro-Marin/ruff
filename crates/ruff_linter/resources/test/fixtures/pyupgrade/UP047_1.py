from typing_extensions import TypeVar

T = TypeVar("T", default=int)


def default_var(var: T) -> T:
    return var
