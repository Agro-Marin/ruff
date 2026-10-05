from odoo import fields, models


class Company(models.Model):
    _inherit = 'res.company'

    planted_config_id = fields.Many2one(comodel_name='planted.config')
