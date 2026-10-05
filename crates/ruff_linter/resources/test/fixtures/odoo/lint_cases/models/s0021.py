def process(self, records):
    for record in records:
        def helper():
            return self.env['res.partner'].search([('active', '=', True)])
