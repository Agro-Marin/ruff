def create_column(cr, tablename, columntype):
    if not tablename:
        raise ValueError(tablename)
    cr.execute(SQL(columntype))
