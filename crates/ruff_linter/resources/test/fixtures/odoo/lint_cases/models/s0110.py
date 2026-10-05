class Rule(models.Model):
    _name = "x.rule"
    amount_min = fields.Float()
    date_min = fields.Date()
    date_max = fields.Date()
