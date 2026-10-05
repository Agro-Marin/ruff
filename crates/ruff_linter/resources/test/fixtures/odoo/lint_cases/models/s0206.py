class A:
    def _query(self):
        return "select 1"

class B:
    def _query(self):
        return "select " + user_input

class C:
    def run(self):
        self.env.cr.execute(A()._query())
