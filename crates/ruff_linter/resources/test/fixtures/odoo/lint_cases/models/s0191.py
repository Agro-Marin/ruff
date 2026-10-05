def observe(self, value):
    statement = SQL("UPDATE t SET v = %s", value)

    def record():
        with self.env.registry.cursor() as cr:
            cr.execute(statement)

    return record
