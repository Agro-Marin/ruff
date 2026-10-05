def do_the_thing(self, env, cr, table):
    self.cr.execute("SELECT * FROM " + table)
