def do_the_thing(self):
    query = "select thing from %s"
    self.env.cr.execute(query % self._table)
