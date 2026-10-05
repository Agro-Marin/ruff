
class Move(models.Model):
    kind = fields.Selection(
        [("23", "Credit note"), ("30", "Debit note"), ("23", "Inactive")],
    )
    other = fields.Selection(selection=[("a", "A"), ("a", "B")])
    clean = fields.Selection([("a", "A"), ("b", "B")])
    dynamic = fields.Selection(selection="_selection_dynamic")
