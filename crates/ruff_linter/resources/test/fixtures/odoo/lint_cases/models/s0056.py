class Provider(models.Model):
    x_secret_key = fields.Char(groups="base.group_system")
    x_webhook_token = fields.Text()
