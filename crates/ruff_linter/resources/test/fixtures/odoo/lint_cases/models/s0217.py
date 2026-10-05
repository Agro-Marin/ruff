def test_function11(self, arg):
    my_injection_variable = "aaaaaaaa" + arg
    self.env.cr.execute('select * from hello where id = %s' % my_injection_variable)
