def do_the_thing(self):
    self.env.cr.execute(f'select name from {self._table}')
