from odoo import fields, models


class Rule(models.Model):
    _name = "fixture.rule"

    score_min = fields.Integer()  # E8532
    score_max = fields.Integer()
    min_amount = fields.Monetary()  # E8532
    max_amount = fields.Float()
    weight_min = fields.Float()  # OK: no partner
    size_min = fields.Char()  # OK: not numeric
    size_max = fields.Integer()
    _min = fields.Integer()  # OK: private, not a field declaration
    _max = fields.Integer()


class Band(models.Model):
    _name = "fixture.band"
    _inherit = "mixin.band"

    value_min = fields.Float()  # OK: the band owns the range
    value_max = fields.Float()


class Score(models.Model):
    _name = "fixture.score"
    _inherit = ["mixin.score.grade"]

    value_min = fields.Float()  # OK
    value_max = fields.Float()
