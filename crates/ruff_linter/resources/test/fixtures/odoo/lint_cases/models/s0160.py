import os as system
from os import environ
environ["GH_TOKEN"] = token
system.environ.setdefault("API_KEY", key)
system.putenv("DB_PASSWORD", pw)
env = system.environ
env["AWS_SECRET_ACCESS_KEY"] = secret
system.environ |= {"SERVICE_TOKEN": token}
from os import putenv as set_variable
set_variable("SMTP_PASSWORD", pw)
set_variable("TZ", "UTC")
