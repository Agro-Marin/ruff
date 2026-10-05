def do_the_thing(self):
    self.env.cr.execute("select thing from %s" % self._table)
