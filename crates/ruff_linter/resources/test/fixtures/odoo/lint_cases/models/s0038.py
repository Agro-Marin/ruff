def process(self):
    while True:
        results = self.env['res.partner'].search([], limit=100)
        if not results:
            break
