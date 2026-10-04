from odoo import models


class Order(models.Model):
    _name = "fixture.order"

    def per_record(self):
        for order in self:
            order.env["sale.order"].search([("partner_id", "=", order.partner_id.id)])  # E8507
            self.env["res.partner"].search_count([])  # E8507

    def nested(self, partners):
        for partner in partners:
            for line in partner.line_ids:
                self.env["account.move"]._read_group([], ["partner_id"])  # E8507, once

    def comprehension(self):
        return [self.env["res.partner"].search_read([]) for record in self]  # E8507

    def over_a_list(self, values):
        for value in values:
            value.search([])  # OK: `search` on a non-ORM receiver
            self.env["res.partner"].search_fetch([], ["name"])  # E8507: always an ORM read

    def hook(self, env):
        for record in env["res.partner"].search([]):
            env["res.partner"].search([("id", "=", record.id)])  # E8507: env model over records

    def chained(self):
        for record in self:
            records = (
                self.env["res.partner"]
                .search([])  # noqa: E8507  one query per company
            )

    def hoisted(self):
        partners = self.env["res.partner"].search([])  # OK: outside the loop
        for partner in partners:
            yield partner.name

    def deferred(self):
        for record in self:

            def later():
                return self.env["res.partner"].search([])  # OK: a function body runs later

            yield later
