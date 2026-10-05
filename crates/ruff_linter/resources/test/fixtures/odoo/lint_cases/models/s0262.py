def f(self, op, column):
    return SQL(  # pylint: disable=sql-injection
        "%s(%s)", SQL(op.value), column
    )
