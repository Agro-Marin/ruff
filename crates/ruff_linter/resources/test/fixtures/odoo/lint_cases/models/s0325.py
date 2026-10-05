import json

from odoo import http


class Planted(http.Controller):
    @http.route('/planted', type='http', auth='user')
    def planted(self):
        return json.dumps({})
