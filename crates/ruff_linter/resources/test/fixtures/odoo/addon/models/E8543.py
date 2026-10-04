from odoo import models


def post_init(env):
    env["res.partner"].unlink()  # E8543
    env["res.partner"].sudo().with_context(active_test=False).write({"active": False})  # E8543
    env["res.config.settings"].update({"group_x": True})  # E8543


class Order(models.Model):
    _name = "fixture.order"

    def clean(self):
        self.env["fixture.line"].action_archive()  # E8543
        self.env[self._name].copy()  # E8543
        self.env["fixture.line"].search([]).unlink()  # OK: the records it names
        self.env["fixture.line"].browse(self.ids).write({})  # OK
        self.lines.unlink()  # OK
        self.env["res.config.settings"].create({}).execute()  # OK
