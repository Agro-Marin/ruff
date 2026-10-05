def f(self):
    partners = self.env['res.partner'].search([('id', 'in', self.ids)])
    return partners
