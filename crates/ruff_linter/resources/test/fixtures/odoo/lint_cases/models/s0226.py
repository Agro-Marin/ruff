def do_the_thing(self, env, cr, table):
    cr.execute("SELECT * FROM " + table)
