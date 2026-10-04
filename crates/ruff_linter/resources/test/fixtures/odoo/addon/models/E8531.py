from odoo import models


class IrHttp(models.AbstractModel):
    _inherit = "ir.http"

    @classmethod
    def _auth_method_partner_token(cls):  # E8531
        return None

    @classmethod
    def _auth_method_public(cls):  # OK: an override of an owned scheme
        return super()._auth_method_public()

    def _auth_method_helper(self):  # E8531
        return None
