from odoo import http
from odoo.http import route


class Api(http.Controller):
    @http.route("/api/orders", type="json2", auth="user")
    def orders(self, limit):  # E8533: not typed
        return []

    @route("/api/hook", auth="bearer", typed=True)
    def hook(self, order_id: int, note, *, flag):  # E8533: two parameters undeclared
        return []

    @http.route("/api/receiver", auth="receiver", typed=True)
    def receiver(self, payload: dict, **kwargs):  # OK: every parameter declared
        return []

    @http.route("/api/ok", type="json2", auth="user", typed=True)
    def ok(self) -> list:  # OK
        return []

    @http.route("/page", type="http", auth="user")
    def page(self, value):  # OK: a browser route
        return ""

    @http.route("/api/one", type="json2", typed=1)
    @http.route("/api/two", auth="bearer", typed=True)
    def twice(self, value):  # E8533 x2: each route is judged
        return ""
