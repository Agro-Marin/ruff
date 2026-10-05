def return_tuple(var):
    return 'a', var

def injectable4(var):
    a, _ = return_tuple(var)
    cr.execute(a)
