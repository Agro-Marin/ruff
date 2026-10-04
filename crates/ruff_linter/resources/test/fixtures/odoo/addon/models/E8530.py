from odoo import fields, models


class Company(models.Model):
    _inherit = "res.company"

    _CREDENTIAL_FIELDS = {"api_key": "fixture"}
    _credential_holder_field = "vault_id"

    sale_note = fields.Text()  # E8530
    sale_discount = fields.Float(compute="_compute_discount", store=True)  # E8530: stored
    sale_amount = fields.Float(compute="_compute_amount", inverse="_inverse_amount")  # E8530: writable
    sale_flag = fields.Boolean(related="partner_id.active", store=STORE)  # E8530: store may be true

    sale_config_id = fields.Many2one(comodel_name="sale.company.config")  # OK: the link
    sale_terms = fields.Text(related="sale_config_id.terms")  # OK: read through the config
    order_ids = fields.One2many(comodel_name="sale.order", inverse_name="company_id")  # OK
    sale_total = fields.Float(compute="_compute_total")  # OK: reads without writing
    sale_ratio = fields.Float(compute="_compute_ratio", store=False)  # OK
    api_key = fields.Char()  # OK: a credential door
    vault_id = fields.Many2one(comodel_name="credential.credential")  # OK: the vault link


class Partner(models.Model):
    _inherit = ["res.partner"]

    sale_note = fields.Text()  # OK: not the company
