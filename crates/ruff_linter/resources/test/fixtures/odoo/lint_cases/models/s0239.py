def test_function1(self, arg):
    my_injection_variable = f"aaaaa{arg}aaa"
    self.env.cr.execute('select * from hello where id = %s' % my_injection_variable)
