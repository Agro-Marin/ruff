import os
os.environ["ANTHROPIC_API_KEY"] = key
os.environ.setdefault("GH_TOKEN", token)
os.environ.update({"DB_PASSWORD": pw})
os.putenv("AWS_SECRET_ACCESS_KEY", secret)
