def create_column(cr, columntype):
    if not _SQL_TYPE_TOKEN.fullmatch(columntype):
        raise ValueError(columntype)
    cr.execute(SQL(columntype))
