def process(self, records):
    for record in records:
        Partner.search([('id', '=', record.id)])
