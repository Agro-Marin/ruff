def do_the_thing(self, env, cr, table):
    env.cr.execute("SELECT * FROM " + table)
