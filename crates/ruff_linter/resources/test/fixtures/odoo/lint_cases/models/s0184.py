def create_column(cr, columntype):
    if columntype == "BOOLEAN":
        columntype = "bool"
    cr.execute(SQL(columntype))
