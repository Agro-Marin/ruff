from odoo import http


class Planted(http.Controller):
    @http.route('/planted/<int:record_id>', type='http', auth='link', link='planted.model:record_id')
    def planted(self, record_id, access_token=None):
        return access_token
