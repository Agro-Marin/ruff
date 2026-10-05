def f(self, user):
    q = 'SELECT 1'
    if (q := user):
        pass
    self.env.cr.execute(q)
