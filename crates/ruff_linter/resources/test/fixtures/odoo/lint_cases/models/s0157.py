class C(http.Controller):
    @http.route("/x", type="jsonrpc", auth="public")
    def x(self):
        raise NotFound()

    def helper(self):
        return NotFound()

    @http.route("/y", auth="public")
    def y(self):
        def fallback():
            return BadRequest()
        return request.render("t")
