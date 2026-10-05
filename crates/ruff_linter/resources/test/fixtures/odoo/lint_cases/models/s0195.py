def do_thing(self, user_input):
    self.env.cr.execute("SELECT * FROM t WHERE x = '%s'" % user_input)
