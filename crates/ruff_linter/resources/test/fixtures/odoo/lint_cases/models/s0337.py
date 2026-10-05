from odoo import http


class Planted(http.Controller):
    @http.route('/hook', type='http', auth='public', csrf=False)
    def hook(self):
        return ''
