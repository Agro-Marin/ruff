def method(self, vars):
    self.env._("something %s %s", *vars)
