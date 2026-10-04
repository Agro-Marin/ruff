from odoo.tests import TransactionCase


class TestOrder(TransactionCase):
    def test_loop(self):
        for record in self.env["res.partner"].search([]):
            self.env["res.partner"].search([("id", "=", record.id)])  # OK: tests are not checked
