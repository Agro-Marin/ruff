
class M(models.Model):
    tag_ids = fields.Many2many(
        comodel_name="m.tag",
        index=True,
    )
    total = fields.Float(
        compute="_compute_total",
        index=True,
    )
    kind = fields.Selection(
        selection=[("a", "A")],
        compute="_compute_kind",
        precompute=True,
    )
    partner_name = fields.Char(
        related="partner_id.name",
        compute="_compute_partner_name",
    )
    stored = fields.Float(
        compute="_compute_stored",
        precompute=True,
        store=True,
        index=True,
    )
    unrelated = fields.Char(
        related=False,
        compute="_compute_unrelated",
    )
    extended = fields.Selection(
        required=True,
        precompute=True,
    )
