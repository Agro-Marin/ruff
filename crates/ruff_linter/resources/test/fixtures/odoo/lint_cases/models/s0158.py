class C(http.Controller):
    @http.route("/x", type="jsonrpc", auth="public")
    def x(self):
        if not self.ok():
            return request.prepare_not_found_error()
        return {}

    @route("/y", auth="public")
    def y(self):
        return werkzeug.exceptions.Forbidden()
