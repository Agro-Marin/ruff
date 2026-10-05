class MyModel(models.Model):
    def unlink(self):
        self._check_delete()
        return super().unlink()
