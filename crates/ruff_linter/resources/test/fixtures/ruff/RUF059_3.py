def foo():
    ...


def bar():
    a = foo()
    b, c = foo()


def baz():
    d, _e = foo()
    print(d)


def qux():
    f, _ = foo()
    print(f)


def quux():
    g, h = foo()
    print(g, h)


def quuz():
    _i, _j = foo()
