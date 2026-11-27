pass
'''
this is not a docstring
'''

l = []

class Cls(MakeKlass('''
    class params \t not a docstring
''')):
    pass

    '''
    this is not a docstring
    '''

    # The colon in the list indexing below is an edge case for the docstring scanner
    def f(self, bar='''
        definitely not a docstring''',
        val=l[Cls():3]):
        some_expression = 'hello world'

        '''
        this is not a docstring
        '''

        if l:
            '''
            Looks like a docstring, but in reality it isn't - only modules, classes and functions
            '''
            pass
