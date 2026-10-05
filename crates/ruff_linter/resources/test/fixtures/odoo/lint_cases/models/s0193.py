def _read(self, table):
    query = "SELECT id FROM "
    query += table
    self.env.cr.execute(query)
