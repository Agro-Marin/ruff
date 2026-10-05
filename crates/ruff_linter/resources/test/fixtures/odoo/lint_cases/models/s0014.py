def process(self, records):
    for record in records:
        wizard.env['crm.team'].search([('id', '=', wizard.id)])
