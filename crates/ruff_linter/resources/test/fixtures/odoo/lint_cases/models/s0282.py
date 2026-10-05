class MyModel(models.Model):
    def unlink(self):
        try:
            return super().unlink()
        except psycopg.Error:
            self._log_failure()
            raise
