
class Partner(models.Model):
    show_credit_limit = fields.Boolean(groups="a")
    use_credit_limit = fields.Boolean()

    show_credit_limit = fields.Boolean(groups="b")
