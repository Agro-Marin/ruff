class Tier(models.Model):
    _name = "x.tier"
    _inherit = ["mixin.catalog", "mixin.band"]
    min_value = fields.Float()
    max_value = fields.Float()

class Grade(models.Model):
    _name = "x.grade"
    _inherit = ["mixin.score.scale"]
    min_value = fields.Float()
    max_value = fields.Float()

class Level(models.Model):
    _name = "x.level"
    _inherit = "mixin.score.scale"
    min_value = fields.Float()
    max_value = fields.Float()
