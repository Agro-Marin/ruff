from odoo import fields, models


class Website(models.Model):
    _inherit = "website"

    google_maps_api_key = fields.Char()  # OK: judged not secret in website
    google_secret_key = fields.Char()  # E8520
