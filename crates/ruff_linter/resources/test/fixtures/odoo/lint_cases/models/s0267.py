for model in ("account.tax", "account.tax.group"):
    cls.env[model].search([("company_id", "=", company.id)])
