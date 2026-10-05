def f(self):
    for record in self:
        record.env['res.partner'].search([('id', '=', record.id)])
