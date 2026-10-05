def f(self, logs):
    used = self.browse(d.id for d in self if logs.search_count([('d', '=', d.id)]))
    named = {m: self.env[m].search_count([]) for m in ('a.b', 'c.d')}
    counts = [self.env['x'].search_count([('p', '=', r.id)]) for r in self.line_ids]
