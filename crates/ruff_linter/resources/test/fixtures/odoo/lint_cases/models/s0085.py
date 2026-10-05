
class A(models.Model):
    _order = "name"
    _order = "id"
    name = fields.Char()

class B(models.Model):
    name = fields.Char()
