from itertools import starmap

map = {}
for _ in starmap(print, zip("A", "12")):
    pass
