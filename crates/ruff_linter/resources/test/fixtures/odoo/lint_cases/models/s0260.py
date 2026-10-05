def f(self, t):
    self.env.cr.execute(
        f"SELECT {t}"
    )  # noqa: E8501  the table name comes from _table
