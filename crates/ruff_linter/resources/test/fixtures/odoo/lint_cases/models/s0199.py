def f(self, user):
    q = 'SELECT 1'
    q: str = user
    self.env.cr.execute(q)
