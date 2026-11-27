from functools import cached_property


class Foo:
    # Bad: property docstrings starting with getter/return verbs

    @property
    def bar(self) -> str:
        return self._bar

    @property
    def baz(self) -> str:
        return self._baz

    @property
    def qux(self) -> str:
        return self._qux

    @property
    def quux(self) -> str:
        return self._quux

    @property
    def corge(self) -> str:
        return self._corge

    @property
    def grault(self) -> str:
        return self._grault

    @property
    def garply(self) -> str:
        return self._garply

    @property
    def waldo(self) -> str:
        return self._waldo

    @property
    def fred(self) -> str:
        return self._fred

    @property
    def plugh(self) -> str:
        return self._plugh

    # Bad: multiline property docstring starting with a verb

    @property
    def multiline_bad(self) -> str:
        return self._value

    # Good: attribute-style property docstrings

    @property
    def good_attr_style(self) -> str:
        return self._bar

    @property
    def good_noun(self) -> str:
        return self._something

    @property
    def good_multiline(self) -> str:
        return self._value

    @cached_property
    def good_cached(self) -> int:
        return 42


# Regular (non-property) methods starting with "Returns" should NOT be flagged

class Bar:
    def regular_method(self) -> str:
        return self._bar

    def another_method(self) -> str:
        return self._value


def top_level_function() -> str:
    """Returns a value."""
    return "value"
