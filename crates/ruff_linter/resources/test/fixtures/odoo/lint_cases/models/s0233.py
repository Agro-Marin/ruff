def test_function3(self, arg):
    my_injection_variable = "aaaaaaaa".format()
    self.env.cr.execute('select * from hello where id = %s' % my_injection_variable)
