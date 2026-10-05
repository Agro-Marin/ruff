def process(self, records):
    for record in records:
        partners = self.env['res.partner'].search([('id', '=', record.id)])
