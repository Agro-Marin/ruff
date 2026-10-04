from odoo.tools import SQL


def bad(cr, ids):
    cr.execute("SELECT id FROM res_partner WHERE id IN %s", [tuple(ids)])  # E8534
    cr.execute("SELECT now() - interval %(days)s", {"days": "1 day"})  # E8534
    query = "UPDATE res_partner SET active = false WHERE id in %s"  # E8534
    cr.execute(query, [ids])
    cr.executemany(query, [[ids]])  # OK: the literal is reported once
    cr.execute(f"SELECT id FROM {table} WHERE id IN %s", [ids])  # E8534: f-string text


def concatenated(cr, ids):
    cr.execute(
        "SELECT id FROM res_partner "
        f"WHERE company_id = {company} "
        "AND id IN %s",  # E8534: one constant from the second piece on
        [ids],
    )


def augmented(cr, ids):
    query = "SELECT id FROM res_partner WHERE TRUE"
    query += " AND id IN %s"  # E8534
    cr.execute(query, [ids])


def good(cr, ids):
    cr.execute(SQL("SELECT id FROM res_partner WHERE id IN %s", tuple(ids)))  # OK: SQL()
    cr.execute("SELECT id FROM res_partner WHERE id = ANY(%s)", [list(ids)])  # OK
    query = SQL("SELECT id FROM res_partner WHERE id IN %s", tuple(ids))
    cr.execute(query)  # OK: SQL() builds it
    cr.execute("SELECT 'MIN %s'", ["x"])  # OK: not the keyword
    cr.execute(other_query, [ids])  # OK: assigned nowhere in scope


def scoped(cr, ids):
    def inner():
        query = "SELECT id WHERE id IN %s"  # OK: another scope
        return query

    cr.execute(query, [ids])
