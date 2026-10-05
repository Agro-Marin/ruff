from odoo.tools import consteq


def f(record, api_token):
    return consteq(record.api_token, api_token)
