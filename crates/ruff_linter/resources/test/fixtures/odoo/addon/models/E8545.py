from odoo import models

LINE = "42"
LINE_ID = int(LINE) if LINE.isdigit() else 0  # E8545: module level


class Partner(models.Model):
    _inherit = "res.partner"

    def _ids_from(self, raw):
        return [int(part) for part in raw.split(",") if part.isdigit()]  # E8545

    def _one(self, value):
        if not value.isdigit():  # E8545: the conversion is later
            return None
        return self.browse(int(value))

    def _stringified(self, value):
        return int(value) if str(value).isdigit() else None  # E8545

    def _defaulted(self, pid):
        if str(pid or "").isdigit():  # E8545
            return int(pid)
        return None

    def _stripped(self, pid):
        return int(pid.strip()) if pid.strip().isdigit() else 0  # E8545

    def _subscripted(self, searches):
        if searches["type"].isdigit():  # E8545: the same subscript, quoted alike or not
            return int(searches['type'])
        return None

    def _in_a_lambda(self, rows):
        return self.browse(int(key) for (key,) in rows if key.isdigit())  # E8545

    def _checksum(self, vat):
        if len(vat) != 11 or not vat.isdigit():  # E8545: a character of it
            return False
        return sum(int(vat[i]) for i in range(10)) % 11 == int(vat[10])

    def _decimal(self, value):
        return int(value) if value.isdecimal() else None

    def _ascii_digits(self, value):
        return int(value) if value.isascii() and value.isdigit() else None

    def _no_conversion(self, value):
        return value.isdigit() and len(value) == 8

    def _another_value(self, value, other):
        return int(other) if value.isdigit() else None

    def _filtered_characters(self, ref):
        return "".join(char for char in ref if char.isdigit())

    def _another_function(self, value):
        return value.isdigit()

    def _converts(self, value):
        return int(value)
