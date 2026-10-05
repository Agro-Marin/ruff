class A:
    @singledispatchmethod
    def f(self, value):
        pass

    @f.register
    def f(self, value: int):
        pass
