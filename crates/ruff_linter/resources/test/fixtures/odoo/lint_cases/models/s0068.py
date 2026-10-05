
class Wizard(models.TransientModel):
    date = fields.Date(default=secrets.token_hex(16))
