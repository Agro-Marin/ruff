def process(self, records, others):
    for record in records:
        pass
    else:
        for other in others:
            self.env['res.partner'].search([('id', '=', other.id)])
