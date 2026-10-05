from odoo import http


class Planted(http.Controller):
    @http.route('/hook', type='http', auth='receiver', receiver='planted.model:_receiver', csrf=False, typed=True)
    def hook(self):
        return ''
