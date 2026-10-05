class Grade(models.Model):
    _name = "x.grade"
    score_min: float = fields.Float()
    score_max: float = fields.Float()
