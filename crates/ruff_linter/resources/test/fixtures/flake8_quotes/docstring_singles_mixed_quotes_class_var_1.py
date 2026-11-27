class SingleLineDocstrings():
    pass
    ''' Not a docstring '''

    def foo(self, bar='''not a docstring'''):
        ''"Start with empty string" ' and lint docstring safely'
        pass

    class Nested(foo()[:]): pass
