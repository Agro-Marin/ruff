def f(self, ids):
    self.env.cr.execute('SELECT 1 FROM x WHERE id = ANY(%s)', (list(ids),))
