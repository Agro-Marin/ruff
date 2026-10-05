def build(coltype):
    assert coltype in TYPES
    return SQL(coltype)
