import hmac

from odoo import models


class Device(models.Model):
    _name = "fixture.device"

    def check(self, presented, signature, key, payload):
        if self.api_token == presented:  # E8536
            return True
        if signature != hmac.new(key, payload, "sha256").hexdigest():  # E8536
            return False
        if self._generate_refresh_token() == presented:  # E8536
            return True
        expected = self._sign_token(payload)
        if presented == expected:  # E8536: a local holding a computed secret
            return True
        if self.state == "done":  # OK
            return False
        if presented == API_TOKEN:  # OK: a named constant
            return False
        if self.access_token == presented:  # OK: a link token is E8537's
            return False
        if self.headers["api_token"] == presented:  # E8536: a subscript by a token name
            return True
        return self.name == presented  # OK
