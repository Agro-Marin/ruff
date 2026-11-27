from typing import AnyStr, Generic

str = "string"


class BadStr(Generic[AnyStr]):
    var: AnyStr
