import smtplib
from imaplib import IMAP4, IMAP4_SSL
from odoo.libs import netguard
class PinnedSMTP(smtplib.SMTP):
    def _get_socket(self, host, port, timeout):
        return netguard.dial(self.addresses, port, timeout=timeout)
class PinnedSMTP_SSL(smtplib.SMTP_SSL, PinnedSMTP):
    pass
class Mailbox(IMAP4):
    def _create_socket(self, timeout):
        return netguard.dial(self.addresses, self.port, timeout=timeout)
class MailboxSSL(Mailbox, IMAP4_SSL):
    def _create_socket(self, timeout):
        return self.ssl_context.wrap_socket(super()._create_socket(timeout))
class Rebound(smtplib.SMTP_SSL, PinnedSMTP):
    def _get_socket(self, host, port, timeout):
        return self.sock
class WrongHook(smtplib.SMTP):
    def _create_socket(self, timeout):
        return netguard.dial(self.addresses, self.port, timeout=timeout)
class ByName(IMAP4):
    def _create_socket(self, timeout):
        return self.open_by_name(self.host)
