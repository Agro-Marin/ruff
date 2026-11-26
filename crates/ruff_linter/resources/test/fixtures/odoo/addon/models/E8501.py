from psycopg import sql

from odoo import models
from odoo.tools import SQL

TABLE = "res_partner"
ORDER = get_order()


def late_query(table):
    return "SELECT * FROM %s" % table


class Partner(models.Model):
    _inherit = "res.partner"

    def literals(self, ids):
        self.env.cr.execute("SELECT id FROM res_partner WHERE id = ANY(%s)", [ids])  # OK
        self.env.cr.execute("SELECT id FROM %s" % self._table)  # OK: _table
        self.env.cr.execute("SELECT id FROM %s" % TABLE)  # OK: a module constant
        self.env.cr.execute("SELECT id FROM res_partner LIMIT %d" % len(ids))  # OK: %d
        self._cr.execute(f"SELECT {self._fields['name'].name} FROM res_partner")  # E8501: a subscript hides the field receiver
        self.env.cr.execute(sql.SQL("SELECT {}").format(sql.Identifier("x")))  # OK: psycopg
        SQL("SELECT %s", ids)  # OK
        self.env.cr.execute("SELECT 1", query="ignored")  # OK

    def injections(self, value, ids):
        self.env.cr.execute("SELECT id FROM res_partner WHERE name = '%s'" % value)  # E8501
        self.env.cr.execute("SELECT id FROM res_partner WHERE name = '" + value + "'")  # E8501
        self.env.cr.execute("SELECT {} FROM res_partner".format(value))  # E8501
        self.env.cr.execute(f"SELECT id FROM res_partner WHERE name = '{value}'")  # E8501
        cursor.execute("SELECT %s" % self.name)  # E8501: a record's name
        SQL("SELECT %s" % value)  # E8501
        self.pool.cursor().execute("SELECT %s" % value)  # OK: not a known cursor spelling
        self.env.cr.execute(query="SELECT %s" % value)  # E8501: the query keyword
        self.env.cr.get_rows_autocommit("SELECT %s" % value)  # E8501: any receiver

    def names(self, value, flag):
        query = "SELECT id FROM res_partner WHERE name = %s" % value
        self.env.cr.execute(query)  # E8501: resolved through its one binding
        clause = "active" if flag else "TRUE"
        self.env.cr.execute("SELECT id FROM res_partner WHERE %s" % clause)  # OK: both branches constant
        column = "name"
        column += value
        self.env.cr.execute("SELECT %s FROM res_partner" % column)  # E8501: one binding is not constant
        for table in ("res_partner", "res_users"):
            self.env.cr.execute("SELECT id FROM %s" % table)  # OK: the loop iterates constants
        first, second = "a", value
        self.env.cr.execute("SELECT %s" % first)  # OK: its tuple position is constant
        self.env.cr.execute("SELECT %s" % second)  # E8501
        self.env.cr.execute("SELECT %s" % ORDER)  # OK: a call the module does not define is trusted

    def guarded(self, direction):
        assert direction in ("ASC", "DESC")
        self.env.cr.execute("SELECT id FROM res_partner ORDER BY id %s" % direction)  # OK: asserted

    def helpers(self, value):
        self.env.cr.execute("SELECT id FROM %s" % self._helper_table())  # OK: returns a constant
        self.env.cr.execute("SELECT id FROM %s" % self._bad_helper(value))  # E8501: _bad_helper returns its argument
        self.env.cr.execute(late_query(value))  # E8501: a bare name is the module's function, not a method

    def _helper_table(self):
        return "res_partner"

    def _bad_helper(self, value):
        return value

    def early(self, value):
        self.env.cr.execute("SELECT %s" % self._later(value))  # E8501: _later, defined below, is judged here

    def _later(self, value):
        return "x" + value

    def params(self, value):
        self.env.cr.execute("SELECT %s" % self._echo("constant"))  # OK: constant arguments
        self.env.cr.execute("SELECT %s" % self._echo(value))  # E8501: a non-constant argument

    def _echo(self, argument):
        return argument

    def _columns_above(self):
        return [name for name in self._fields if self[name]]

    def through_a_builder(self):
        self.env.cr.execute(build_query(self._columns_above()))  # OK: a call the module does not define is trusted
        self.env.cr.execute(build_query(self._columns_below()))  # OK: the same, whatever the order

    def _columns_below(self):
        return [name for name in self._fields if self[name]]


def cast_by(cast):
    return SQL("CAST(NULL AS %s)" % cast("x"))  # OK: cast is this parameter, not the function cast_owner defines


def cast_owner():
    def cast(name):
        return name

    return cast


def wrapped(value):
    def wrap(text):
        return text

    return SQL("SELECT %s" % wrap(value))  # E8501: the nested function returns its argument
