class C(http.Controller):
    @http.route("/x", type="jsonrpc", auth="public")
    def x(self):
        return json.dumps({"id": 1})

    @http.route("/y", type="http", auth="public")
    def y(self):
        return request.prepare_json_response({"id": 1})

    @http.route("/z", type="http", auth="public")
    def z(self):
        def inner():
            return json.dumps({})
        return inner()
