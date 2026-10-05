class A:
    @typing.overload
    def f(self, x: int) -> int: ...

    @typing.overload
    def f(self, x: str) -> str: ...

    def f(self, x):
        return x
