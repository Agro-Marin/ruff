from odoo import fields, models


class Order(models.Model):
    _name = "fixture.order"

    partner_name = fields.Char(related="partner_id.name", store=True)  # E8529

    image_128 = fields.Image(related="image_1920", max_width=128, store=True)  # OK: a resize
    file = fields.Binary(related="attachment_id.datas", store=True)  # OK
    country = fields.Char(related="partner_id.country_id.name")  # OK: not stored
    city = fields.Char(related="partner_id.city", store=1)  # OK: not the literal True
    kept = fields.Char(related="partner_id.ref", store=True)  # noqa: E8529  the unique constraint reads it
