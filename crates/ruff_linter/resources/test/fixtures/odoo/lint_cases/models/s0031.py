def process(self, records):
    for record in records:
        self.search([('id', '=', record.id)])
        self.env['res.partner'].search([('id', '=', record.id)])
        self.sudo().search([('id', '=', record.id)])
