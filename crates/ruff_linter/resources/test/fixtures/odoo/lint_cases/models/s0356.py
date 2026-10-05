from odoo import http


class Planted(http.Controller):
    @http.route('/api/planted', type='json2', auth='bearer', typed=True)
    def planted(self, value: int):
        return value
