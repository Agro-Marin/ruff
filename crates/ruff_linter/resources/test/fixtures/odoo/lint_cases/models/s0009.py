def process(self, records):
    for record in records:
        comodel.with_context(active_test=False).search([])
