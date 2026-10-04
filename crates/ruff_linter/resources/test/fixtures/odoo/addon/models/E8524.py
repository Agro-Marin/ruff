from odoo import fields, models


class Order(models.Model):
    _name = "fixture.order"

    total = fields.Float(compute="compute_total")  # E8524
    name = fields.Char(
        compute="_compute_name",
        inverse="set_name",  # E8524 (reported on the call)
        search="find_name",  # E8524 (reported on the call)
    )
    state = fields.Selection(selection="get_states")  # E8524

    amount = fields.Float(compute="_compute_amount", inverse="_inverse_amount")  # OK
    kind = fields.Selection(selection="_selection_kind")  # OK
    level = fields.Integer(compute=compute_level)  # OK: not a method name
