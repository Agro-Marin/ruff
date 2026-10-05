def observe(self, table):
    statement = f"UPDATE {table} SET v = 1"

    def record():
        with self.env.registry.cursor() as cr:
            cr.execute(statement)

    return record
