UserError('This is not translated')
exceptions.UserError('This is also not translated')
UserError(f'This is an f-string')
raise UserError('This is not translated' + 'This is also not translated')
some_var = 'random_string'
raise UserError('This is not translated' and some_var)
raise UserError('This is not translated' if true else _('This is translated'))
