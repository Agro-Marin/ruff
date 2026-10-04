from odoo import models


class Order(models.Model):
    _name = "fixture.order"

    def action(self):
        self.ensure_one()  # E8535
        return self.env["ir.actions.actions"]._for_xml_id("fixture.action")  # E8535

    def checked(self):
        self.check_singleton()  # OK
        ensure_one()  # OK: not a method call
