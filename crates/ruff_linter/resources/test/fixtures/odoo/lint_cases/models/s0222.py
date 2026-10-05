def _init_column1(self, column_name):
    query = 'SELECT %(var1)s FROM %(var2)s WHERE %(var3)s' % {'var1': 'field_name', 'var2': 'table_name', 'var3': 'where_clause'}
    self.env.cr.execute(query)
