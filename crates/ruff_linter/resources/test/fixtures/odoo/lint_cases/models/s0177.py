def f(cr, ids):
    query = "SELECT id FROM t WHERE id IN %s"
    cr.execute(query, (ids,))
    cr.execute(query, (ids,))
