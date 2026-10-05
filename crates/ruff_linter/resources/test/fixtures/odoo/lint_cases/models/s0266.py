taxes = product.supplier_taxes_id.filtered(
    lambda t: t.active and t.company_id in company.parent_ids
)
