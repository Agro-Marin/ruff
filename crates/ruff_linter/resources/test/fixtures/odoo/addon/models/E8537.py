from odoo import models
from odoo.tools import consteq


class Order(models.Model):
    _name = "fixture.order"

    def open(self, token):
        if self.access_token == token:  # E8537
            return self
        if consteq(self.booking_token, token):  # E8537
            return self
        if self.push_token == token:  # OK: a device's credential
            return None
        if consteq(self.access_token, self.iot_token):  # OK: the stored side is a credential
            return None
        return self.search([("access_token", "=", token)])  # E8537

    def lookups(self, token):
        self.search_count([("order_id.document_token", "in", [token])])  # E8537
        self.search([("access_token", "=", False)])  # OK: rows without a token
        self.search([("api_token", "=", token)])  # OK: not a link
        return self._generate_access_token() == token  # E8537: a call is skipped, the other side is a token
