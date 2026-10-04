from odoo import http
from odoo.http import request


def _check(record, access_token):
    return record._from_access_token(access_token)


class Portal(http.Controller):
    @http.route("/my/order/<int:order_id>", type="http", auth="public")
    def order(self, order_id, access_token=None):  # E8539
        order = request.env["sale.order"].browse(order_id)
        if order.access_token != access_token:
            return request.not_found()
        return order

    @http.route("/my/invoice/<int:invoice_id>", type="http", auth="link", link="account.move:invoice_id")
    def invoice(self, invoice_id, access_token=None):  # OK: declares auth="link"
        return request.link_subject

    @http.route("/my/quote/<int:quote_id>", type="http", auth="public")
    def quote(self, quote_id, access_token=None):  # OK: reaches a door through _check
        return _check(request.env["sale.order"].browse(quote_id), access_token)

    @http.route("/my/plain", type="http", auth="public")
    def plain(self, access_token=None):  # OK: never reads the token
        return ""

    @http.route
    def bare(self, access_token=None):  # OK: no route arguments
        return access_token

    @http.route("/my/kw", type="http", auth="public")
    def keyword_only(self, *, access_token):  # E8539
        return access_token

    def legacy(self, record):
        return record._document_check_access("sale.order", record.id)  # E8539
