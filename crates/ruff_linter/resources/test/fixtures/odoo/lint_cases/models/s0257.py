def test1(self):
    operator = 'aaa'
    value = 'bbb'
    op1, val1 = (operator, value)
    self.env.cr.execute('query' + op1)
