
class Wizard(models.TransientModel):
    date = fields.Date(default=fields.Date.today())
