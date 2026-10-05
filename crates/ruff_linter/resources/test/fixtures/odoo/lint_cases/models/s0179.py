cr.execute(SQL("SELECT id FROM t WHERE id IN %s", tuple(ids)))
