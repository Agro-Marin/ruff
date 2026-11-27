# OK
def foo(_) -> None:
    ...


# OK
def foo(*_) -> None:
    ...


# OK
def foo(**_) -> None:
    ...


# OK
def foo(a: int, _) -> None:
    ...


# OK
def foo() -> None:
    def bar(_) -> None:
        ...
