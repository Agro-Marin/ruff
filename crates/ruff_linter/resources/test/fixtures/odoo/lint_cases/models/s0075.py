
class M(models.Model):
    company_id = fields.Many2one(
        related="order_id.company_id",
        store=True,
    )
    currency_id = fields.Many2one(
        related="order_id.currency_id",
    )
    image_128 = fields.Image(
        related="image_1920",
        max_width=128,
        store=True,
    )
    state = fields.Selection(
        related="order_id.state",
        store=False,
    )
