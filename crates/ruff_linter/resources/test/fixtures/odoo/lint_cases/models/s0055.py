class Settings(models.TransientModel):
    x_client_secret = fields.Char(config_parameter="x.client_secret")
    x_wizard_password = fields.Char()
