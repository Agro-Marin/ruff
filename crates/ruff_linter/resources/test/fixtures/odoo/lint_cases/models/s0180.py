cr.execute(SQL("SELECT now() - INTERVAL %s", SQL.literal(offset)))
