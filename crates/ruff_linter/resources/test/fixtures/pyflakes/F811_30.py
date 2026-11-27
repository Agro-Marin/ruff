class A:
    def foo(self) -> None:
        pass

    bar = foo

    def bar(self) -> None:
        pass


class B:
    def baz(self) -> None:
        pass

    baz = 1


class C:
    def foo(self) -> None:
        pass

    bar = (foo := 1)


class D:
    foo = 1
    foo = 2
    bar = (foo := 3)
    bar = (foo := 4)
