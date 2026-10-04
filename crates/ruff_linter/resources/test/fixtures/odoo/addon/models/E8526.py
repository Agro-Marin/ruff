from odoo import fields, models


class Order(models.Model):
    _name = "fixture.order"

    name = fields.Char(
        required=True,
        string="Name",  # E8526: string= comes before required=
    )
    note = fields.Text(
        help="Help",
        groups="base.group_user",  # E8526: groups= comes before help=
    )
    code = fields.Char(
        string="Code",
        zeta=1,
        alpha=2,  # E8526: unknown keywords sort alphabetically
    )
    tail = fields.Char(
        groups="base.group_user",
        unknown=True,  # E8526: unknown keywords come before the tail
    )
    inline = fields.Char(string="Inline", required=True)  # E8526: keywords on the call's line
    shared = fields.Char(
        string="Shared", required=True,  # E8526: two keywords on a line
    )
    spread = fields.Char(
        string="Spread",
        **EXTRA, required=True,  # E8526: an unpacking shares the line
    )

    ordered = fields.Char(
        string="Ordered",
        required=True,
        alpha=1,
        zeta=2,
        groups="base.group_user",
        help="Help",
    )  # OK
    single = fields.Char(string="Single")  # OK: one keyword may share the line
    with_unpacking = fields.Char(
        string="Unpacked",
        **EXTRA,
    )  # OK
