class Settings(models.TransientModel):
    def _inverse_x_client_secret(self):
        ICP = self.env["ir.config_parameter"].sudo()
        ICP.set_param("x.client_secret", self.x_client_secret)

    def _read(self):
        ICP = self.env["ir.config_parameter"].sudo()
        return ICP.get_param("x.private_key"), ICP.get_param("x.client_id")
