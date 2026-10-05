def process(self, items):
    for item in items:
        record._read_group([('x', '=', item)])
