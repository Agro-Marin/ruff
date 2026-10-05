def f(self, t):
    self.env.cr.execute(
        f"SELECT {t}"
    )
    other()  # noqa: E8501  a different statement
