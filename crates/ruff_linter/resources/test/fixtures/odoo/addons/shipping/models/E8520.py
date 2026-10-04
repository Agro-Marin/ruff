from odoo import fields, models


class Carrier(models.Model):
    _inherit = "delivery.carrier"

    shipping_api_key = fields.Char()  # E8520
    shipping_password = fields.Text(store=True, compute="_compute_password")  # E8520: stored
    shipping_client_secret = fields.Char(related="account_id.secret", store=1)  # OK: related, not literally stored
    shipping_token_type = fields.Char()  # OK: about a token
    shipping_api_key_url = fields.Char()  # OK
    shipping_secret_hash = fields.Char()  # OK: derived
    shipping_public_key = fields.Char()  # OK: published
    shipping_sync_token = fields.Char()  # OK: a cursor
    share_token = fields.Char()  # OK: a link capability
    cache_key = fields.Char()  # OK: not a credential
    shipping_secret = fields.Char(compute="_compute_secret")  # OK: not stored
    shipping_key = fields.Integer()  # OK: not a string field
    signing_key = fields.Char()  # OK: the module hashes it

    def _sign(self, value):
        self.signing_key = hashlib.sha256(value).hash()

    def _keys(self):
        ICP = self.env["ir.config_parameter"].sudo()
        ICP.get_param("shipping.api_key")  # E8520
        ICP.set_param("shipping.webhook_secret", "x")  # E8520
        ICP.get_param("database.secret")  # OK: judged not secret
        ICP.get_param("shipping.api_endpoint")  # OK


class Settings(models.TransientModel):
    _inherit = "res.config.settings"

    shipping_api_token = fields.Char(config_parameter="shipping.api_token")  # E8520
    shipping_password = fields.Char()  # OK: a transient field without a parameter
