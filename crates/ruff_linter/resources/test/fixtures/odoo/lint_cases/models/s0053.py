def _read(env):
    ICP = env["ir.config_parameter"].sudo()
    ICP.get_param("database.secret")
    ICP.get_param("mail.web_push_vapid_public_key")
    ICP.get_param("x.token_endpoint")
