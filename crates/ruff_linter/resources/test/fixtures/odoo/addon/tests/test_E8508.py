import odoo.orm  # E8508
import odoo.orm.fields as orm_fields  # E8508
from odoo import orm  # E8508
from odoo.orm import models  # E8508
from odoo.orm.fields import Char  # E8508
from typing import TYPE_CHECKING

from odoo import api, fields, models  # OK: the façade
from odoo.tools import orm_helpers  # OK: another package

if TYPE_CHECKING:
    from odoo.orm.environments import Environment  # OK: typing only
