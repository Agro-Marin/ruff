class C(http.Controller):
    @route("/x", auth="public")
    def x(self):
        return json.dumps([])
