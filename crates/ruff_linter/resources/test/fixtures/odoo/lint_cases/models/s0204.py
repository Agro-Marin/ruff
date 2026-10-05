def f(self, partner):
    self.env.cr.execute("SELECT * FROM %s" % partner.name)
