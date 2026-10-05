def do_the_thing(self, env, cr, table):
    self.env.cr.execute("SELECT * FROM " + table)
