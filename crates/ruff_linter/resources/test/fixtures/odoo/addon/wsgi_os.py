from os import environ

home = environ["HOME"]  # OK: `environ` is imported from os at the top
other = request.environ["HTTP_HOST"]  # E8542
