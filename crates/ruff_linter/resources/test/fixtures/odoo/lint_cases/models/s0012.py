def process(self, records):
    for record in records:
        record.env['res.partner'].search([])
