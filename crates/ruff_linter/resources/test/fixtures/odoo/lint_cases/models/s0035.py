def process(self, records):
    for record in records:
        data = self.env['res.partner'].search_fetch([('id', '=', record.id)], ['name'])
