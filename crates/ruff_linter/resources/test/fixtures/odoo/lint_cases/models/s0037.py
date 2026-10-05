def process(self, records):
    partners = self.env['res.partner'].search([('active', '=', True)])
    for record in records:
        partner = partners.filtered(lambda p: p.id == record.id)
