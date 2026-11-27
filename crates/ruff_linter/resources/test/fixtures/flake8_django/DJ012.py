from django.db import models
from django.db.models import Model


class StrBeforeRandomField(models.Model):
    class Meta:
        verbose_name = "test"
        verbose_name_plural = "tests"

    def __str__(self):
        return ""

    random_property = "foo"


class StrBeforeFieldModel(models.Model):
    class Meta:
        verbose_name = "test"
        verbose_name_plural = "tests"

    def __str__(self):
        return "foobar"

    first_name = models.CharField(max_length=32)


class ManagerBeforeField(models.Model):
    objects = "manager"

    class Meta:
        verbose_name = "test"
        verbose_name_plural = "tests"

    def __str__(self):
        return "foobar"

    first_name = models.CharField(max_length=32)


class CustomMethodBeforeStr(models.Model):
    class Meta:
        verbose_name = "test"
        verbose_name_plural = "tests"

    def my_method(self):
        pass

    def __str__(self):
        return "foobar"


class GetAbsoluteUrlBeforeSave(Model):
    def get_absolute_url(self):
        pass

    def save(self):
        pass


class ConstantsAreNotFields(models.Model):
    first_name = models.CharField(max_length=32)

    class Meta:
        verbose_name = "test"
        verbose_name_plural = "tests"

    def __str__(self):
        pass

    MY_CONSTANT = id(1)


class PerfectlyFine(models.Model):
    first_name = models.CharField(max_length=32)
    last_name = models.CharField(max_length=32)
    objects = "manager"

    class Meta:
        verbose_name = "test"
        verbose_name_plural = "tests"

    def __str__(self):
        return "Perfectly fine!"

    def save(self, **kwargs):
        super(PerfectlyFine, self).save(**kwargs)

    def get_absolute_url(self):
        return "http://%s" % self

    def my_method(self):
        pass

    @property
    def random_property(self):
        return "%s" % self


class MultipleConsecutiveFields(models.Model):
    class Meta:
        verbose_name = "test"

    first_name = models.CharField(max_length=32)
    last_name = models.CharField(max_length=32)

    def get_absolute_url(self):
        pass

    middle_name = models.CharField(max_length=32)


class BaseModel(models.Model):
    pass


class StrBeforeFieldInheritedModel(BaseModel):
    class Meta:
        verbose_name = "test"
        verbose_name_plural = "tests"

    def __str__(self):
        return "foobar"

    first_name = models.CharField(max_length=32)


# https://github.com/astral-sh/ruff/issues/13892
class DunderMethodOtherThanStrBeforeSave(models.Model):
    name = models.CharField()

    def __init__(self, *args, **kwargs): ...

    def save(*args, **kwargs): ...
