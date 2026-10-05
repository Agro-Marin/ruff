cr.execute("SELECT id FROM t WHERE id = ANY(%s)", (ids,))
