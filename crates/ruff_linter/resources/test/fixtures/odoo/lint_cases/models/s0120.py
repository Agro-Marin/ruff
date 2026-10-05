ok = consteq(record.access_token, token)
order = env['pos.order'].sudo().search([('access_token', '=', t)])
row = env['x'].search([('document_token', 'in', ts)], limit=1)
