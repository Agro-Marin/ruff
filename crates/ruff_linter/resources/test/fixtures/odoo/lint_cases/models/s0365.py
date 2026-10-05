from odoo import _, api, fields, http, models, tools


class Planted(models.Model):
    _name = "planted.model"

    kind = fields.Selection(
        selection=[('a', 'A'), ('a', 'B')],
    )
