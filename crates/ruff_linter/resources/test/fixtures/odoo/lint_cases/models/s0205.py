def f(self, field):
    self.env.cr.execute("SELECT %s FROM t" % field.name)
