class SingleLineDocstrings():
    pass
    """ Not a docstring """

    def foo(self, bar="""not a docstring"""):
        "Do not"' start with empty string' ' and lint docstring safely'
        pass

    class Nested(foo()[:]): pass
