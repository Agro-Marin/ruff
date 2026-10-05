def do_the_thing(self, env, cr, table):
    self._cr.execute("SELECT * FROM " + table)
