def _q(self):
    def key(rec):
        return rec.partner_id.street
    return "SELECT 1"
def run(self):
    self.env.cr.execute(self._q())
