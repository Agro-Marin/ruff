class MyModel(models.Model):
    def unlink(self):
        if self.state == 'posted':
            raise UserError("Cannot delete posted record")
        return super().unlink()
