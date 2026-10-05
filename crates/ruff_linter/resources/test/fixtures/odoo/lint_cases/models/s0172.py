class A:
    if typing.TYPE_CHECKING:
        def f(self) -> int: ...
    else:
        def f(self):
            return 1
