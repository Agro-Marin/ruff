
class M(models.Model):
    line_ids = fields.One2many("m.line", "m_id", "Lines")
    tag_ids = fields.Many2many("m.tag", "rel", "a", "b")
