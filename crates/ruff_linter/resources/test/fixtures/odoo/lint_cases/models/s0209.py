def read(self, table):
    return db_connect(self.dbname).get_rows_autocommit(
        "SELECT * FROM " + table
    )
