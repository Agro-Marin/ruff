from ty_extensions import Intersection, Not
from ty_extensions._internal import (
    CallableTypeOf,
    RegularCallableTypeOf,
    TypeOf,
)


class A: ...


class B: ...


def _(x: Not[A]):
    pass


def _(x: Intersection[A], y: Intersection[A, B]):
    pass


def _(x: TypeOf[1j]):
    pass


def _(x: CallableTypeOf[str]):
    pass


def _(x: RegularCallableTypeOf[str]):
    pass
