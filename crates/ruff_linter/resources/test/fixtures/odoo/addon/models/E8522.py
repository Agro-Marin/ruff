import uuid

from odoo import _, fields, models


class Order(models.Model):
    _name = "fixture.order"

    date = fields.Date(default=fields.Date.today())  # E8522
    moment = fields.Datetime(default=fields.Datetime.now())  # E8522
    token = fields.Char(default=uuid.uuid4())  # E8522
    label = fields.Char(default=_("New"))  # E8522

    day = fields.Date(default=fields.Date.today)  # OK: the callable
    stamp = fields.Datetime(default=lambda self: fields.Datetime.now())  # OK
    reference = fields.Char(default=compute_reference())  # OK: not a per-record call
    other = fields.Char(default="x", required=True)  # OK
