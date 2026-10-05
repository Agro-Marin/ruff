class C(http.Controller):
    @http.route("/x", type="http", auth="public")
    def x(self):
        return json_dumps({"id": 1})
