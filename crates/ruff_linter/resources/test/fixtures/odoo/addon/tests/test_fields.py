from odoo import fields, models


class Fixture(models.Model):
    _name = "fixture.test"

    name = fields.Char("Name")  # OK: tests are not checked
    name = fields.Char("Name")
    partner_name = fields.Char(related="partner_id.name", store=True)
