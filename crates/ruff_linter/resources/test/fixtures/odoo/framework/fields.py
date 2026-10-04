from odoo import fields, models


class Framework(models.Model):
    _name = "fixture.framework"

    name = fields.Char("Name")  # E8525: outside an addon too
    partner_name = fields.Char(related="partner_id.name", store=True)  # E8529
