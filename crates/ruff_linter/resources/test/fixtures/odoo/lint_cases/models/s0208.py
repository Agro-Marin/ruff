def f(self, x, y):
    a, b, c = (*x, y)
    self.env.cr.execute(c)
