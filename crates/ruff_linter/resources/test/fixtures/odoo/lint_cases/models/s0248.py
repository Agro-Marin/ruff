def _search_phone_mobile_search(self, operator, value):
    condition = 'IS NULL' if operator == '=' else 'IS NOT NULL'
    query = '''
        SELECT model.id
        FROM %s model
        WHERE model.phone %s
        AND model.mobile %s
    ''' % (self._table, condition, condition)
    self.env.cr.execute(query)
