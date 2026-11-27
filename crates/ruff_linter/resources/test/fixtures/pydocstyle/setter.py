class PropertyWithSetter:
    @property
    def foo(self) -> str:
        return "foo"

    @foo.setter
    def foo(self, value: str) -> None:
        pass

    @foo.deleter
    def foo(self):
        pass

    @foo
    def foo(self, value: str) -> None:
        pass
