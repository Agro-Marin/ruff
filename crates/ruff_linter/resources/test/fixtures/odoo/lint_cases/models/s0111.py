class Grade(models.Model):
    _name = "x.grade"
    score_min = fields.Integer()
    score_max = fields.Integer()
    min_age = fields.Float()
    max_age = fields.Float()
