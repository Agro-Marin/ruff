from markupsafe import Markup


def f(value):
    return Markup('<b>%s</b>' % value)
