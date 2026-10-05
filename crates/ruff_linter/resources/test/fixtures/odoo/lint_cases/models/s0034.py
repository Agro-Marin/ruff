def process(self, records):
    for record in records:
        count = self.env['res.partner'].search_count([('id', '=', record.id)])
