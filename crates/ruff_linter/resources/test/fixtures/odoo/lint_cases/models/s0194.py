query = "SELECT 1"
def f(self, query, /):
    self.env.cr.execute(query)
