def f(request):
    return request.httprequest.environ['HTTP_USER_AGENT']
