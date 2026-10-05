def process(self, records):
    callbacks = []
    for record in records:
        callbacks.append(lambda: self.env['res.partner'].search([]))
