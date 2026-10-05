class MyModel(models.Model):
    def unlink(self):
        raise UserError("nope")
        return super().unlink()
