from odoo import models


class Line(models.Model):
    _name = "fixture.line"

    def taxes(self, company):
        self.tax_ids.filtered(lambda tax: tax.company_id == company)  # E8514
        self.product_id.taxes_id.filtered(lambda t, c=company: t.company_id.id == c.id)  # E8514
        self.tax_ids.filtered(lambda tax: tax.company_ids & company.parent_ids)  # OK
        self.line_ids.filtered(lambda line: line.company_id == company)  # OK: not taxes
        self.tax_ids.filtered(lambda tax: other.company_id)  # OK: not the parameter

    def domains(self, company):
        self.env["account.tax"].search([("company_id", "=", company.id)])  # E8514
        self.env["account.account"].sudo().search_count(
            domain=["|", ["company_id", "=", False], ("active", "=", True)],  # E8514
        )
        for model in ("account.tax.group", "account.journal"):
            self.env[model].search([("company_id", "in", company.ids)])  # E8514
        self.env["account.journal"].search([("company_id", "=", company.id)])  # OK
        self.env["account.tax"].search([("company_ids", "in", company.ids)])  # OK
