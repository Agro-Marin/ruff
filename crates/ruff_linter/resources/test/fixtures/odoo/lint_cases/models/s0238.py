def test_function2(self):
    arg = 'bbb'
    my_injection_variable = f"aaaaa{arg}aaa"
    self.env.cr.execute('select * from hello where id = %s' % my_injection_variable)
