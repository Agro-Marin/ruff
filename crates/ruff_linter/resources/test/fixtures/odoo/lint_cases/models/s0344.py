def f(self, ids):
    self.env['res.partner'].browse(ids).write({'active': False})
