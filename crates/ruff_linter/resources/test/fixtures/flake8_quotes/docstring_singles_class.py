class SingleLineDocstrings():
    pass
    ''' Not a docstring '''

    def foo(self, bar='''not a docstring'''):
        ''' Double quotes single line method docstring'''
        pass

    class Nested(foo()[:]): pass
