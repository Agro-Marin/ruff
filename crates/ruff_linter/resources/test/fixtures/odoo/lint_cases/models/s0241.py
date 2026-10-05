class A:
    def f(self):
        return self.g()

    def g(self):
        return self.f()

    def run(self):
        self.env.cr.execute("SELECT 1 " + self.f())
