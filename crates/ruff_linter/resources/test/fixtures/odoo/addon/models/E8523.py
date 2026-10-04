from odoo import fields, models


class Order(models.Model):
    _name = "fixture.order"

    state = fields.Selection(
        selection=[("draft", "Draft"), ("done", "Done"), ("draft", "Again")],  # E8523
    )
    kind = fields.Selection([("a", "A"), ("b", "B"), ("a", "A2"), ("b", "B2")])  # E8523 x2
    level = fields.Selection(
        selection=(("low", "Low"), ["low", "Lower"], ("x", "y", "z"), ("x", "X")),  # E8523
    )
    target = fields.Reference(selection=[("res.partner", "P"), ("res.partner", "Q")])  # E8523

    unique = fields.Selection(selection=[("a", "A"), ("b", "B")])  # OK
    triples = fields.Selection(selection=[("a", "A", 1), ("a", "A", 2)])  # OK: not pairs
    numbers = fields.Selection(selection=[(1, "One"), (1, "Uno")])  # OK: not string keys
    method = fields.Selection(selection="_selection_method")  # OK: a method name
