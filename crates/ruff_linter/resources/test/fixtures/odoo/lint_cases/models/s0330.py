def f(request):
    return request.httprequest.environ.get('HTTP_USER_AGENT')
