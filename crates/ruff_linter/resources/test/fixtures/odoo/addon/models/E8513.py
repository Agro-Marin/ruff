from functools import singledispatchmethod
from typing import overload

from odoo import api, models


class Order(models.Model):
    _name = "fixture.order"

    def action(self):
        return 1

    @api.model
    def action(self):  # E8513
        return 2

    def triple(self):
        return 1

    def triple(self):
        return 2

    async def triple(self):  # E8513: names both earlier lines
        return 3

    @overload
    def parse(self, value: int) -> int: ...

    @overload
    def parse(self, value: str) -> str: ...

    def parse(self, value):  # OK: overload stubs
        return value

    @property
    def amount(self):
        return self._amount

    @amount.setter
    def amount(self, value):  # OK: the property's setter
        self._amount = value

    @property
    def total(self):
        return 0

    @amount.setter
    def total(self, value):  # E8513: re-decorates another property
        pass

    @singledispatchmethod
    def handle(self, value):
        return value

    @handle.register
    def _(self, value: int):
        return value

    @handle.register
    def _(self, value: str):  # OK: registered implementations
        return value

    @handle.register(float)
    def handle(self, value):  # E8513: under the dispatcher's own name it replaces it
        return value

    def outer(self):
        def inner():
            return 1

        def inner():  # OK: not a class body
            return 2

        return inner
