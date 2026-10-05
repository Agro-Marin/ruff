class A:
    @singledispatchmethod
    def f(self, value):
        pass

    @f.register
    def _(self, value: int):
        pass

    @f.register
    def _(self, value: str):
        pass
