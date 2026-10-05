raise UserError(_('This is translated'))
some_var = 'This is not translated'
raise UserError(some_var)
raise UserError(some_var + _('This is translated'))
raise UserError(_('This is translated') and some_var)
raise UserError(_('This is translated') if true else some_var)
def some_call():
    return _("nothing")
some_arr = ["random_string", _("another_random_string")]
raise UserError(some_arr[0])
