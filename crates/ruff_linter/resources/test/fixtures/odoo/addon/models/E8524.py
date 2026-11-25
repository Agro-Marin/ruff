from odoo import fields, models


class Order(models.Model):
    _name = "fixture.order"

    total = fields.Float(compute="compute_total")  # E8524
    name = fields.Char(
        compute="_compute_name",
        inverse="set_name",  # E8524 (reported on the call)
        search="find_name",  # E8524 (reported on the call)
    )
    state = fields.Selection(selection="get_states")  # E8524

    amount = fields.Float(compute="_compute_amount", inverse="_inverse_amount")  # OK
    kind = fields.Selection(selection="_selection_kind")  # OK
    level = fields.Integer(compute=compute_level)  # OK: not a method name


class Report(models.Model):
    _name = "fixture.report"

    stage_id = fields.Many2one(comodel_name="fixture.stage", group_expand="_read_group_stage_ids")  # E8524
    team_id = fields.Many2one(comodel_name="fixture.team", group_expand="_read_group_expand_full")  # OK: the ORM's own
    phase_id = fields.Many2one(comodel_name="fixture.phase", group_expand="_group_expand_phase_id")  # OK
    margin = fields.Float(value_sql="_margin_sql")  # E8524
    rank = fields.Integer(value_sql="_value_sql_rank", order_by_sql="_rank_order")  # E8524 (reported on the call)
    band = fields.Char(group_by_sql="_group_by_sql_band", aggregate_sql="_band_total_sql")  # E8524 (reported on the call)
    total = fields.Float(aggregate_sql="_aggregate_sql_total", order_by_sql="_order_by_sql_total")  # OK
