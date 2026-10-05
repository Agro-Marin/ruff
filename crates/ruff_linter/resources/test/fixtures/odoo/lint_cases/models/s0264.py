groups = self.env["account.tax"].sudo()._read_group(
    domain=[("company_id", "in", self.company_ids.ids)],
    aggregates=["tax_group_id:recordset"],
)
