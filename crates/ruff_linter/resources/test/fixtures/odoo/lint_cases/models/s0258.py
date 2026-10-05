def f(self, a, b):
    self.env.cr.execute(f"S {a}"), self.env.cr.execute(f"S {b}")
