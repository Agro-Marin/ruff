import os
method = request.httprequest.environ["REQUEST_METHOD"]
body = environ["wsgi.input"]
uri = environ["REQUEST_URI"]
ip = request.httprequest.environ.get("REMOTE_ADDR")
environ["QUERY_STRING"] = qs
path = os.environ["PATH"]
