
class Wizard(models.TransientModel):
    date = fields.Date(default=fields.Date.today)
    when = fields.Datetime(default=lambda self: fields.Datetime.now())
    code = fields.Char(default=",".join(CODES))
    since = fields.Datetime(default=datetime(2018, 1, 1))
    label = fields.Char(default=_lt("New"))
