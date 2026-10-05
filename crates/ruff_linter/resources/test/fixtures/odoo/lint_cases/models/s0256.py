def test_function12(self):
    arg1 = "a"
    arg2 = "b" + arg1
    arg3 = arg2 + arg1 + arg2
    arg4 = arg1 + "d"
    my_injection_variable = arg1 + arg2 + arg3 + arg4
    self.env.cr.execute('select * from hello where id = %s' % my_injection_variable)
