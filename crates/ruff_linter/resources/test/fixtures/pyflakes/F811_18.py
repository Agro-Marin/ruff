import unittest, transport


class GetTransportTestCase(unittest.TestCase):
    def test_get_transport(self):
        transport = 'transport'
        self.assertIsNotNone(transport)


class TestTransportMethodArgs(unittest.TestCase):
    def test_send_defaults(self):
        transport.Transport()
