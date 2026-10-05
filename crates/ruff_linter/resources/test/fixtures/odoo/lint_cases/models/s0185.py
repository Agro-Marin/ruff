def build(table):
    return f"SELECT * FROM {table}"

def a(self, t):
    self.env.cr.execute(build(t))

def b(self, t):
    self.env.cr.execute(build(t))
