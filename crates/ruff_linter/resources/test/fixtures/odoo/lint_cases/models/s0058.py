class Provider(models.Model):
    x_api_key = fields.Char(compute="_compute_x_api_key")
    share_token = fields.Char()
    x_publishable_key = fields.Char()
    x_token_hash = fields.Char()
    x_password = fields.Char()

    def _set(self, value):
        self.x_password = crypt_context.hash(value)
