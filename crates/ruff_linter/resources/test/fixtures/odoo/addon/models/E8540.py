from odoo import models, tools
from odoo.tools import ormcache


class Users(models.Model):
    _inherit = "res.users"

    @tools.ormcache("self.env.uid")  # E8540
    def _menus(self):
        return []

    @ormcache("uid", "lang")  # E8540
    def _labels(self, uid, lang):
        return []

    @tools.ormcache("self.env.user.id", "self.env.user.share")  # OK: follows the share flag
    def _portal(self):
        return []

    @tools.ormcache("self.env.access_signature")  # OK
    def _actions(self):
        return []

    @tools.ormcache("self.env.uid", cache="memberships")  # OK: cleared by a membership
    def _groups(self):
        return []

    @tools.ormcache("self.env.lang")  # OK: not per user
    def _lang(self):
        return []

    @tools.ormcache("self.uidx")  # OK: not the uid
    def _other(self):
        return []
