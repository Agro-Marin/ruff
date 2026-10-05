def process(self, records):
    for record in records:
        record.write({'active': True})
        record.unlink()
        name = record.name_get()
