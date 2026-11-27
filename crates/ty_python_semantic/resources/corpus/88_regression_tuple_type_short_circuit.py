from typing_extensions import Never


def never() -> Never:
    return never()


(never(), 2)
