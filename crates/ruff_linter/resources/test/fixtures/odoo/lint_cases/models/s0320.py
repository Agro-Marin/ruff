def f(self, company):
    return self.tax_ids.filtered(lambda t: company in t.company_ids)
