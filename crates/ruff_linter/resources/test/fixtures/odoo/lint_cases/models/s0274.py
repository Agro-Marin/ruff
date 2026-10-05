taxes = self.env["account.tax"].search(
    self.env["account.tax"]._check_company_domain(company)
)
