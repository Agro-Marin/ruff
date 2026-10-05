def hook(env):
    for move in env['account.move'].search([]):
        env['account.tax'].search([('id', '=', move.id)])
    for xml_id in ('a', 'b'):
        env['ir.model.data'].search([('name', '=', xml_id)])
    while True:
        if not env['res.users'].search([], limit=1):
            break
