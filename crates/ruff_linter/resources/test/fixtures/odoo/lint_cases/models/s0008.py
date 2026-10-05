def process(self, records):
    for record in records:
        lead.with_context(active_test=False).search([])
