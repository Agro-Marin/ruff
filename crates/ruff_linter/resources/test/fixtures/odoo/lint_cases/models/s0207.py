def _build(self, where_sql, pids):
    where_clause = where_sql
    if pids:
        where_clause = "(%s) AND (%s)" % (where_clause, "fol.partner_id = ANY(%s)")
    self.env.cr.execute("SELECT id FROM t WHERE " + where_clause)
