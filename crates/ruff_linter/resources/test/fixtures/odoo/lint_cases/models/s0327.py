from odoo.http import request

from odoo import http


class Planted(http.Controller):
    @http.route('/planted', type='http', auth='user')
    def planted(self):
        return request.prepare_not_found_error()
