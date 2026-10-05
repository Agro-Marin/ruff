def f(self, table):
    self.env.cr.execute('SELECT * FROM ' + table)
