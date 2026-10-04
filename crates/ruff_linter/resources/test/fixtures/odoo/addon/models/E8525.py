from odoo import fields, models


class Order(models.Model):
    _name = "fixture.order"

    name = fields.Char("Name")  # E8525
    partner_id = fields.Many2one("res.partner", "Partner")  # E8525
    line_ids = fields.One2many("fixture.order.line", "order_id")  # E8525
    state = fields.Selection([("a", "A")], "State")  # E8525
    amount = fields.Float("Amount", (16, 2), 4, "extra")  # E8525
    spread = fields.Char(*ARGUMENTS)  # E8525

    label = fields.Char(string="Label")  # OK
