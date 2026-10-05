rows = env['ir.attachment'].search([('access_token', '=', False)])
rows = env['x'].search([('access_token', 'in', None)])
gone = subs.filtered(lambda s: consteq(s.push_token, token))
same = subscription.push_token == token
rows = env['x'].search([('access_token', '=', value)])
