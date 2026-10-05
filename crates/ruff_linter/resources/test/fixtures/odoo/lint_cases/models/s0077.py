
class M(models.Model):
    a = fields.Char(required=True, string="A")
    b = fields.Char(string="B", required=True)
    c = fields.Char(
        string="C",
        required=True,
    )
    d = fields.Char(string="D")
    e = fields.Char(string="E",
                    required=True)
