
class Users(models.Model):
    totp_enabled = fields.Boolean(compute="_compute_totp_enabled", search="_totp_enable_search")
    cert = fields.Binary(compute="_compute_cert", inverse="_set_cert")
    kind = fields.Selection(selection="_get_kinds")
    fine = fields.Char(compute="_compute_fine", inverse="_inverse_fine", search="_search_fine")
    shared = fields.Float(compute="_compute_amounts")
