# OK
def foo(num: int) -> str:
    """
    Do something

    Parameters
    ----------
    num : int
        A number
    """
    print('test')


# DOC202
def foo(num: int) -> str:
    """
    Do something

    Parameters
    ----------
    num : int
        A number

    Returns
    -------
    str
        A string
    """
    print('test')


class Bar:

    # DOC202
    def foo(self) -> str:
        print('test')


    # OK
    def bar(self) -> str:
        print('test')


import abc


class A(metaclass=abc.abcmeta):
    @abc.abstractmethod
    def f(self):
        return
