def f(self, company):
    return self.tax_ids.filtered(lambda t: t.company_id == company)
