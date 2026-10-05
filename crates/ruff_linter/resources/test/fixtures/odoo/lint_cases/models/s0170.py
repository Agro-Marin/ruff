class A:
    def f(self):
        pass

    @some_registry.register
    def f(self):
        pass
