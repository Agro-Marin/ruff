from odoo import fields, models


class Partner(models.Model):
    _inherit = "res.partner"

    name = fields.Char()
    name = fields.Char(required=True)  # E8521: a field over a field

    code = fields.Char()
    code = "constant"  # E8521: a plain value over a field

    label = "constant"
    label = fields.Char()  # E8521: a field over a plain value

    flag = True
    flag = False  # OK: no field involved

    first = second = fields.Integer()
    second = fields.Integer()  # E8521: chained targets bind both names

    amount: float = fields.Float()
    amount: float = fields.Float(digits=(16, 2))  # E8521: annotated assignments count

    note: str  # OK: a bare annotation binds nothing
    note = fields.Text()

    def method(self):
        name = fields.Char()  # OK: a function body is not the class body
        name = fields.Char()
        return name


class Other(models.Model):
    _name = "fixture.other"

    name = fields.Char()  # OK: each class has its own namespace
