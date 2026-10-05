from odoo import _, api, fields, http, models, tools


class Planted(models.Model):
    _name = "planted.model"

    @api.onchange('a')
    def _onchange_a(self):
        return {'warning': {'title': 'x', 'message': 'y'}}
