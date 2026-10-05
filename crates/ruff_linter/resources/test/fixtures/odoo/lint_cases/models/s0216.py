def test():
    arg = "test"
    arg = arg + arg
    self.env.cr.execute(arg)
