from odoo import fields, models


class Company(models.Model):
    _inherit = 'res.company'

    planted_flag = fields.Boolean()
