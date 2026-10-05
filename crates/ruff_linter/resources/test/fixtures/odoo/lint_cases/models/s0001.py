class IrHttp(models.AbstractModel):
    _inherit = "ir.http"

    @classmethod
    def _auth_method_outlook(cls):
        pass

    @classmethod
    def _auth_method_public(cls):
        pass
