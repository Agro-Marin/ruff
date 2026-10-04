from odoo import fields, models


class Order(models.Model):
    _name = "fixture.order"

    line_ids = fields.One2many(comodel_name="fixture.line", inverse_name="order_id", index=True)  # E8527
    tag_ids = fields.Many2many(comodel_name="fixture.tag", index="btree")  # E8527
    total = fields.Float(compute="_compute_total", index=True)  # E8527
    amount = fields.Float(compute="_compute_amount", precompute=True)  # E8527
    stored = fields.Float(compute="_compute_stored", store=0, precompute=True)  # E8527
    name = fields.Char(related="partner_id.name", compute="_compute_name")  # E8527
    both = fields.Char(compute="_compute_both", index=True, precompute=True)  # E8527 x2

    kept = fields.Float(compute="_compute_kept", store=True, index=True, precompute=True)  # OK
    plain = fields.Char(index=True)  # OK: not computed here
    unindexed = fields.One2many(comodel_name="fixture.line", inverse_name="order_id", index=False)  # OK
    blank = fields.Many2many(comodel_name="fixture.tag", index="")  # OK
    unrelated = fields.Char(related="", compute="_compute_unrelated")  # OK: a falsy related
