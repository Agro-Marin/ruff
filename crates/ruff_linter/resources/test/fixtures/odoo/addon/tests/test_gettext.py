from odoo import _, _lt, models
from odoo.exceptions import AccessError, UserError, ValidationError


class Order(models.Model):
    _name = "fixture.order"

    def variable(self, message):
        _(message)  # E8502
        self.env._(f"Order {self.name}")  # E8502: an f-string is not a literal
        _lt()  # E8502: no literal at all
        _("Order %s", self.name)  # OK

    def placeholders(self):
        _("%s of %s")  # E8503
        _("%i of %i")  # E8503: every conversion Python accepts
        _("%%%s and %s")  # E8503: an escaped percent, then two
        _("%(done)s of %(total)s")  # OK: named
        _("100%% of %s")  # OK: one placeholder
        _("%%s and %s")  # OK: the first is an escaped percent
        _("%b and %n")  # OK: not conversions Python accepts

    def representation(self):
        _("Bad value %r")  # E8504
        _("Bad value %(value)r")  # E8504
        _("Bad value %%r")  # OK: escaped
        _("%r and %s")  # E8503 and E8504

    def missing(self, value):
        raise UserError("The order is locked")  # E8505
        raise ValidationError("Locked: " + value)  # E8505: a word in the concatenation
        raise AccessError(f"No access to {value}")  # E8505
        raise UserError(value)  # OK: a name
        raise UserError(self.env._("Locked"))  # OK
        raise UserError(value + ": " + str(value))  # OK: no word outside values
        raise UserError(f"{value}: %s")  # OK: only a placeholder
        raise UserError(value if value else self.name)  # OK
        raise UserError(value or "Locked")  # E8505: a constant in the BoolOp
        raise UserError(*value)  # E8505: a starred argument

    def developer(self, value):
        raise ValueError(_("Bad value"))  # E8511
        raise KeyError(_("Missing %s") % value)  # E8511
        raise RuntimeError(_lt("Failed {}").format(value))  # E8511
        raise ValueError(f"Bad value {value}")  # OK
        raise ValueError(value)  # OK
