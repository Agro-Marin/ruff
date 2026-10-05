class Hooks(http.Controller):
    @http.route("/hook", type="http", auth="public", csrf=False)
    def hook(self, **kw):
        return self._handle(request.get_json_data())

    @route("/hook/<id>", type="http", auth="none", csrf=False)
    def resolved(self, id, **kw):
        device = request.env["x"].search([("id", "=", id)])
        device.admit()
        return device
