import requests
from zeep.transports import Transport
from imaplib import IMAP4_SSL
class TimeoutSession(requests.Session):
    pass
class Plain(object):
    pass
class Mailbox(IMAP4_SSL):
    pass
