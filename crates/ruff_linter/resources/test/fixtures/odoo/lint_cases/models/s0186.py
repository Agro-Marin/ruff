def a(self, t):
    self.env.cr.execute(build(t))

def b(self, t):
    self.env.cr.execute(build(t))

def c(self, t):
    self.env.cr.execute(build(t))

def build(table):
    return f"SELECT * FROM {table}"
