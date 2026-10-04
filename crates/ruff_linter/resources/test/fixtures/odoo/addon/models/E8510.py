from unittest import mock
from unittest.mock import patch

from odoo.tools import config

with patch.dict(config.options, {"workers": 2}):  # E8510
    pass
with mock.patch.dict(odoo.tools.config.options, workers=2):  # E8510
    pass
with patch.dict("odoo.tools.config.options", {"workers": 2}):  # E8510
    pass
with patch.dict("config.options", {"workers": 2}):  # E8510
    pass

with patch.dict(os.environ, {"X": "1"}):  # OK: not the configuration
    pass
with config.patch(workers=2):  # OK
    pass
with patch.dict(*ARGUMENTS):  # OK: not readable
    pass
