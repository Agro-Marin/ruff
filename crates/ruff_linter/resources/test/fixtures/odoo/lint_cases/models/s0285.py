class MyModel(models.AbstractModel):
    def unlink(self):
        raise UserError("nope")
        return super().unlink()
