def test_function7(self):
    arg = "aaa"
    my_injection_variable = "aaaaaaaa {test}".format(test="aaa" + arg)
    self.env.cr.execute('select * from hello where id = %s' % my_injection_variable)
