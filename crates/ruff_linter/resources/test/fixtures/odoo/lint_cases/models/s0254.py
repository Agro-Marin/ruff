def f(self, where):
    query = "SELECT * FROM {} WHERE " + where
    self.env.cr.execute(query.format(self._table))
