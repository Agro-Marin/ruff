class ClassSimplest:
    pass
class ClassWithSingleField:
    a = 1
class ClassWithJustTheDocstring:
    pass
class ClassWithInit:
    def __init__(self):
        pass
class ClassWithTheDocstringAndInit:
    def __init__(self):
        pass
class ClassWithInitAndVars:
    cls_var = 100
    def __init__(self):
        pass
class ClassWithInitAndVarsAndDocstring:
    cls_var = 100
    def __init__(self):
        pass
class ClassWithDecoInit:
    @deco
    def __init__(self):
        pass
class ClassWithDecoInitAndVars:
    cls_var = 100
    @deco
    def __init__(self):
        pass
class ClassWithDecoInitAndVarsAndDocstring:
    cls_var = 100
    @deco
    def __init__(self):
        pass
class ClassSimplestWithInner:
    class Inner:
        pass
class ClassSimplestWithInnerWithDocstring:
    class Inner:
        def __init__(self):
            pass
class ClassWithSingleFieldWithInner:
    a = 1
    class Inner:
        pass
class ClassWithJustTheDocstringWithInner:
    class Inner:
        pass
class ClassWithInitWithInner:
    class Inner:
        pass
    def __init__(self):
        pass
class ClassWithInitAndVarsWithInner:
    cls_var = 100
    class Inner:
        pass
    def __init__(self):
        pass
class ClassWithInitAndVarsAndDocstringWithInner:
    cls_var = 100
    class Inner:
        pass
    def __init__(self):
        pass
class ClassWithDecoInitWithInner:
    class Inner:
        pass
    @deco
    def __init__(self):
        pass
class ClassWithDecoInitAndVarsWithInner:
    cls_var = 100
    class Inner:
        pass
    @deco
    def __init__(self):
        pass
class ClassWithDecoInitAndVarsAndDocstringWithInner:
    cls_var = 100
    class Inner:
        pass
    @deco
    def __init__(self):
        pass
class ClassWithDecoInitAndVarsAndDocstringWithInner2:
    class Inner:
        pass
    cls_var = 100
    @deco
    def __init__(self):
        pass
