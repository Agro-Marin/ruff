def planted_check(self, token):
    return self.env['access.link']._resolve(token)
