from odoo import _, api, fields, http, models, tools


class Planted(models.Model):
    _name = "planted.model"

    day = fields.Date(default=fields.Date.today)
