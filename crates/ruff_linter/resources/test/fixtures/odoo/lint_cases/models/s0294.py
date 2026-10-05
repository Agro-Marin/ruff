def f(self, value):
    self.env.cr.execute('SELECT 1 WHERE id = %s', (value,))
