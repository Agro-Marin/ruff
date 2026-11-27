class A:
    def foo(self) -> None:
        """Foo."""

    bar = foo

    def bar(self) -> None:
        """Bar."""


class B:
    def baz(self) -> None:
        """Baz."""

    baz = 1


class C:
    def foo(self) -> None:
        """Foo."""

    bar = (foo := 1)


class D:
    foo = 1
    foo = 2
    bar = (foo := 3)
    bar = (foo := 4)
