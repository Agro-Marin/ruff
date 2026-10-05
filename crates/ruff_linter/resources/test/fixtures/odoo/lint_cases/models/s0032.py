def process(self, records):
    for record in records:
        groups = self.env['sale.order']._read_group(
            [('partner_id', '=', record.id)],
            groupby=['state'],
            aggregates=['amount_total:sum'],
        )
