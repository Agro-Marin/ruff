class C(http.Controller):
    @http.route("/x", type="http", auth="public")
    def x(self):
        if not self.ok():
            return json.dumps({"error": "forbidden"})
        return json.dumps({"id": 1})
