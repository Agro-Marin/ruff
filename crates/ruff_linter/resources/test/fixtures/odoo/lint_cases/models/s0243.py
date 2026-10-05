def test_function9(self, arg):
    my_injection_variable = "aaa" % arg
    self.env.cr.execute('select * from hello where id = %s' % my_injection_variable)
