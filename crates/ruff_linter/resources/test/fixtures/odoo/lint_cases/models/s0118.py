class C:
    @route('/a/<int:x>', auth='public')
    def open_a(self, x, access_token=None):
        return access_token
    @route('/b/<int:x>', auth='link', link='m:x')
    def open_b(self, x, access_token=None):
        return access_token
    @route('/c/<int:x>', type='jsonrpc', auth='public')
    def open_c(self, x, access_token=None):
        return self._check(x, access_token)
    def _check(self, x, token):
        return env['access.link']._open_record('m', x, token)
    @route()
    def open_a_override(self, x, access_token=None):
        return access_token
    @route('/d', auth='public')
    def unused(self, access_token=None):
        return 1
rec = self._document_check_access('m', 1)
