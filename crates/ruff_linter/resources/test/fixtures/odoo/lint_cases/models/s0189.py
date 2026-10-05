def run(self):
    q = "SELECT 1"
    def inner(x):
        q = x
    self.env.cr.execute(q)
