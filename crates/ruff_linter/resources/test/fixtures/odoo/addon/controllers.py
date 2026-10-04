import json

from werkzeug.exceptions import NotFound

from odoo import http
from odoo.http import request, route


class Main(http.Controller):
    @http.route("/json", type="http", auth="user")
    def as_json(self):
        return json.dumps({"ok": True})  # E8515

    @route("/plain", auth="user")
    def plain(self):
        if not request.env.user:
            return dumps({})  # E8515: no type= is an HTTP route
        return json_dumps({})  # E8515

    @http.route("/rpc", type="jsonrpc", auth="user")
    def rpc(self):
        return json.dumps({})  # OK: not an HTTP route

    @http.route("/nested", type="http", auth="user")
    def nested(self):
        def inner():
            return json.dumps({})  # OK: the inner function's return

        return request.prepare_json_response(inner())

    @http.route("/missing", type="http", auth="user")
    def missing(self):
        if not request.env.user:
            return NotFound()  # E8541
        return request.prepare_not_found_error()  # E8541

    @http.route("/raised", type="http", auth="user")
    def raised(self):
        raise NotFound()  # OK

    def helper(self):
        return NotFound()  # OK: not a route

    @http.route("/hook", type="http", auth="public", csrf=False)
    def hook(self):  # E8528
        return ""

    @http.route(
        "/probe",
        type="http",
        auth="none",
        csrf=False,
    )
    async def probe(self):  # E8528
        return ""

    @http.route("/receiver", type="http", auth="receiver", csrf=False)
    def receiver(self):  # OK
        return ""

    @http.route("/form", type="http", auth="public")
    def form(self):  # OK: csrf checked
        return ""

    @http.route("/zero", type="http", auth="public", csrf=0)
    def zero(self):  # OK: not the literal False
        return ""
