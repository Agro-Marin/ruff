def f(self, user):
    q = 'SELECT 1'
    with user as q:
        pass
    self.env.cr.execute(q)
