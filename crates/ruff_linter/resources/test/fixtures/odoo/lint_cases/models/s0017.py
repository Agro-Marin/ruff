def process(self, records):
    for record in records:
        if record.active:
            partners = self.env['res.partner'].search([('id', '=', record.id)])
