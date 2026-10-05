def do_the_thing(cr, name):
    cr.execute('select %s from thing' % name)
