from odoo.tools import consteq


def planted_check(record, token):
    return consteq(record.access_token, token)
