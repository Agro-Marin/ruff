from odoo import _, api, fields, http, models, tools


class Planted(models.Model):
    _name = "planted.model"

    @tools.ormcache('self.env.uid')
    def _planted(self):
        return 1
