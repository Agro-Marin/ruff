def f(self, user):
    q = 'SELECT 1'
    match user:
        case q:
            pass
    self.env.cr.execute(q)
