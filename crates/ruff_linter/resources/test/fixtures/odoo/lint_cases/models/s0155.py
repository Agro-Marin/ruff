class Pages(http.Controller):
    @http.route("/page", type="http", auth="user", csrf=False)
    def page(self):
        return ""

    @http.route("/form", type="http", auth="public")
    def form(self):
        return ""
