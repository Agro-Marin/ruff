taxes = self.env["account.tax"].search(
    self.env["account.tax"]._get_domain_company_compatible(company)
)
