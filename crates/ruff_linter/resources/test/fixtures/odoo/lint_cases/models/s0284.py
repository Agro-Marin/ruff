class MyModel(models.TransientModel):
    def unlink(self):
        raise UserError("nope")
        return super().unlink()
