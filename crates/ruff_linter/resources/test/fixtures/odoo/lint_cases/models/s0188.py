def f(self, n):
    self.env.cr.execute("SELECT 1 LIMIT %(n)d" % {"n": n})
