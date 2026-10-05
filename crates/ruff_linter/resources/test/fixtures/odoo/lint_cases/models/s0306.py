from odoo import _, api, fields, http, models, tools


class Planted(models.Model):
    _name = "planted.model"

    @api.ondelete(at_uninstall=False)
    def _unlink_except_x(self):
        raise UserError(_('no'))
