try:
    pass
except 1:  # Error
    pass

try:
    pass
except (1, ValueError):  # Error
    pass

try:
    pass
except (ValueError, (RuntimeError, (KeyError, TypeError))):  # Error
    pass

try:
    pass
except (ValueError, *(RuntimeError, (KeyError, TypeError))):  # Error
    pass


try:
    pass
except (*a, *(RuntimeError, (KeyError, TypeError))):  # Error
    pass


try:
    pass
except* a + (RuntimeError, (KeyError, TypeError)):  # Error
    pass


try:
    pass
except (ValueError, *(RuntimeError, TypeError)):  # OK
    pass

try:
    pass
except (ValueError, *[RuntimeError, *(TypeError,)]):  # OK
    pass


try:
    pass
except (*a, *b):  # OK
    pass


try:
    pass
except (*a, *(RuntimeError, TypeError)):  # OK
    pass


try:
    pass
except (*a, *(b, c)):  # OK
    pass


try:
    pass
except (*a, *(*b, *c)):  # OK
    pass


def what_to_catch():
    return ...


try:
    pass
except what_to_catch():  # OK
    pass


try:
    pass
except (a, b) + (c, d):  # OK
    pass


try:
    pass
except* (a, b) + (c, d):  # OK
    pass


try:
    pass
except* (a, (b) + (c)):  # OK
    pass


try:
    pass
except (a, b) + (c, d) + (e, f):  # OK
    pass


try:
    pass
except a + (b, c):  # OK
    pass


try:
    pass
except (ValueError, *(RuntimeError, TypeError), *((ArithmeticError,) + (EOFError,))):
    pass


try:
    pass
except ((a, b) + (c, d)) + ((e, f) + (g)):  # OK
    pass

try:
    pass
except (a, b) * (c, d):  # B030
    pass

try:
    pass
except* 1:  # Error
    pass

try:
    pass
except* (1, ValueError):  # Error
    pass

try:
    pass
except* (ValueError, (RuntimeError, (KeyError, TypeError))):  # Error
    pass

try:
    pass
except* (a, b) * (c, d):  # B030
    pass
