cr.execute("SELECT now() - %s", (timedelta(hours=1),))
