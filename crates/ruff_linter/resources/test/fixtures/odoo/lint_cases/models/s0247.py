def _init_column(self, column_name):
    query = f'UPDATE "{self._table}" SET "{column_name}" = %s WHERE "{column_name}" IS NULL'
    self.env.cr.execute(query, (value,))
