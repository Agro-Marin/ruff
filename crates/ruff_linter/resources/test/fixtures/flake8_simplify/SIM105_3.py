def foo():
    pass


def bar():
    # SIM105
    try:
        foo()
    except ValueError:
        pass


import contextlib
