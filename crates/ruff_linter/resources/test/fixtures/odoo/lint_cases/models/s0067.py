
class Wizard(models.TransientModel):
    date = fields.Date(default=uuid.uuid4())
