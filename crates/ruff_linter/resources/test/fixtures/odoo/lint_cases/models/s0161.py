import os
os.environ["TZ"] = "UTC"
env = {**os.environ, "ANTHROPIC_API_KEY": key}
subprocess.run(cmd, env=env)
