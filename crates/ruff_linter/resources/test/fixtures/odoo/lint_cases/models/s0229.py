def test_function4(self, arg):
    my_injection_variable = "aaaaaaaa {test}".format(test="aaa")
    self.env.cr.execute('select * from hello where id = %s' % my_injection_variable)
