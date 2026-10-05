def read(self, sid):
    return self.conn.get_rows_autocommit(
        "SELECT payload FROM http_session WHERE sid = %s", (sid,)
    )
