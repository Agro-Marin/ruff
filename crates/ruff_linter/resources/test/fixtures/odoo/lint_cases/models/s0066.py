
class Wizard(models.TransientModel):
    date = fields.Date(default=datetime.now())
