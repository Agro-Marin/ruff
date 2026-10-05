cr.execute("SELECT id FROM t WHERE id IN %(ids)s", {"ids": ids})
