def _init_column(self, value):
    query = f'UPDATE "{self._table}" SET "{self._rec_name}" = %s'
    self.env.cr.execute(query, (value,))
