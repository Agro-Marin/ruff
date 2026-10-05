import requests


def f(url):
    return requests.get(url, timeout=5)
