from odoo import models
from odoo.exceptions import UserError
from odoo.orm import models as orm_models


class Order(models.Model):
    _name = "fixture.order"

    def unlink(self):
        if self.filtered("locked"):
            raise UserError("Locked orders cannot be deleted")  # E8506
        try:
            return super().unlink()
        except Exception:
            raise  # OK: a bare raise hands on what was refused

    def write(self, vals):
        raise UserError("No")  # OK: not unlink


class Line(orm_models.TransientModel):
    def unlink(self):
        def check():
            raise ValueError("nested")  # E8506: still inside the override

        check()
        return super().unlink()


class Helper:
    def unlink(self):
        raise ValueError("not a model")  # OK
