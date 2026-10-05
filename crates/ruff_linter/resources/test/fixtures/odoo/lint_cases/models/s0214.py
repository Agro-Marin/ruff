def test2(self):
    operator = 'aaa'
    operator += 'bbb'
    self.env.cr.execute('query' + operator)
