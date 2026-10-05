def f(self):
    tbl = "things"
    self.env.cr.execute("SELECT * FROM %s" % tbl)
