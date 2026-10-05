def process(self, records):
    for record in records:
        request.env['sms.tracker'].search([])
