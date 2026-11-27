class SingleLineDocstrings():
    pass
    ''' Not a docstring '''

    def foo(self, bar='''not a docstring'''):
        pass

    class Nested(foo()[:]): pass
