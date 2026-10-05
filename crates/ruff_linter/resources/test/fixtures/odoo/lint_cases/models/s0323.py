from odoo import _, api, fields, http, models, tools


class Planted(models.Model):
    _name = "planted.model"

    score_min = fields.Float()
    score_max = fields.Float()
