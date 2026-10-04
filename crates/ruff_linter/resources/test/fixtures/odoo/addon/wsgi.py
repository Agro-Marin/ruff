
def application(environ, start_response):
    method = environ["REQUEST_METHOD"]  # OK: guaranteed
    remote = environ["REMOTE_ADDR"]  # E8542
    host = request.httprequest.environ["HTTP_HOST"]  # E8542
    agent = environ.get("HTTP_USER_AGENT")  # OK
    home = os.environ["HOME"]  # OK: the process environment
    environ["HTTP_X"] = "1"  # OK: a store
    scheme = environ["wsgi.url_scheme"]  # OK
    return [method, remote, host, agent, home, scheme]
