async def process(self, records):
    async for record in records:
        self.env['res.partner'].search([('id', '=', record.id)])
