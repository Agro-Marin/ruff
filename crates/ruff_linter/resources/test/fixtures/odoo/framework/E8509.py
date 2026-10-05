from odoo import api, models


class Order(models.Model):
    _name = "fixture.order"

    @api.onchange("partner_id")
    def _onchange_partner(self):
        return {"domain": {"user_id": [("partner_id", "=", self.partner_id.id)]}}  # E8509

    @api.onchange("user_id")
    def _onchange_user(self):
        result = {}
        result["domain"] = {"partner_id": []}  # E8509
        result["domain"] = {"partner_id": []}  # OK: one per method
        return result

    @onchange("team_id")
    def _onchange_team(self):
        return dict(domain={"user_id": []})  # E8509

    @api.onchange("state")
    def _onchange_state(self):
        values = {"warning": {"title": "x"}}
        return values["domain"]  # OK: a read, not a store

    def _not_an_onchange(self):
        return {"domain": []}  # OK
