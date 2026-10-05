def f(cr, ids):
    query = SQL("SELECT id FROM t WHERE id IN %s", tuple(ids))
    cr.execute(query)
