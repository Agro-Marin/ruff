def test_function10(self):
    my_injection_variable = "aaa" + "aaa"
    self.env.cr.execute('select * from hello where id = %s' % my_injection_variable)
