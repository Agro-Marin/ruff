def process(self, items):
    for item in items:
        record.search_fetch([('x', '=', item)])
