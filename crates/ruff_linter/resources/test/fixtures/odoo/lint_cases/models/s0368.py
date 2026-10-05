from odoo import _, api, fields, http, models, tools


class Planted(models.Model):
    _name = "planted.model"

    a = fields.Char(compute='_compute_a')
