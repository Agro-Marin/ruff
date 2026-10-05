def test_function5(self):
    arg = 'aaa'
    my_injection_variable = "aaaaaaaa {test}".format(test=arg)
    self.env.cr.execute('select * from hello where id = %s' % my_injection_variable)
