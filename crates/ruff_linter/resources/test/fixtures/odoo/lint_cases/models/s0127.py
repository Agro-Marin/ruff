@api.onchange('a')
def _onchange_a(self):
    self.env['x'].search([('domain', '=', self.domain)])
