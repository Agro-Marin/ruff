def test_function10(self, arg):
    if_else_variable = "aaa" if arg else "bbb"
    self.env.cr.execute('select * from hello where id = %s' % if_else_variable)
