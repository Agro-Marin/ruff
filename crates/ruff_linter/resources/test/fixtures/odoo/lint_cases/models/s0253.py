def _read(self, table):
    self.env.cr.execute(query=f"SELECT id FROM {table}")
