from unittest.mock import patch
from odoo.tools import config

with patch.dict(config.options, {'a': 1}):
    pass
