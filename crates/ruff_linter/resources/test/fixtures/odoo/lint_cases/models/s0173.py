self.env.cr.execute("SELECT id FROM t WHERE id IN %s", (ids,))
