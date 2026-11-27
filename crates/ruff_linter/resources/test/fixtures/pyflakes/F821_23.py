import typing

User = typing.NamedTuple('User', **{'name': str, 'password': bytes})
User = typing.NamedTuple('User', name=str, password=bytes)
User = typing.NamedTuple('User', [('name', str), ('password', bytes)])
