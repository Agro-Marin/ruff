from odoo import http


class Planted(http.Controller):
    @http.route('/api/planted', type='json2', auth='bearer')
    def planted(self, value):
        return value
