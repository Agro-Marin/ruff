from odoo.tests import TransactionCase


class TestCounts(TransactionCase):
    def test_rows(self):
        before = self.cr.sql_log_count  # E8516
        self.env["res.partner"].create([{"name": "a"}, {"name": "b"}])
        self.assertEqual(self.cr.sql_log_count - before, 2)  # E8516
        del self.cr.sql_log_count  # E8516
        self.cr.sql_log_count = 0  # OK: an assignment
        self.assertEqual(self.cr.sql_statement_count, 1)  # OK
