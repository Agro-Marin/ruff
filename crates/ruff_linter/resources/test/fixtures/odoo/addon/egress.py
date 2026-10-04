import imaplib
import os
import smtplib
import urllib.request
from os import environ as env_alias, putenv

import httpx as client
import requests
from requests import Session as RequestsSession

from odoo.libs import netguard


def fetch(url):
    requests.get(url, timeout=5)  # E8518
    client.AsyncClient()  # E8518
    RequestsSession()  # E8518
    urllib.request.urlopen(url)  # E8518
    httpx.get(url)  # OK: httpx is not imported under that name
    client.Timeout(5)  # OK: builds a value


class Mailer(smtplib.SMTP):  # E8518: subclasses a dialing client
    pass


class PinnedSMTP(smtplib.SMTP):  # OK: its socket hook dials through netguard
    def _get_socket(self, host, port, timeout):
        return netguard.dial(host, port, timeout)


class PinnedChild(PinnedSMTP):  # OK: inherits the pinned hook
    pass


class PinnedIMAP(imaplib.IMAP4):  # E8518: the hook does not dial through netguard
    def _create_socket(self, timeout):
        return super()._create_socket(timeout)


def secrets(key):
    os.environ["AWS_SECRET_ACCESS_KEY"] = key  # E8519
    env_alias["API_TOKEN"] = key  # E8519
    os.environ.setdefault("DB_PASSWORD", key)  # E8519
    os.environ.update({"SERVICE_CREDENTIALS": key})  # E8519
    os.environ.update(GITHUB_TOKEN=key)  # E8519
    os.environ |= {"private_key": key}  # E8519: the name is matched in any case
    putenv("SECRET", key)  # E8519
    os.putenv("API_KEY", key)  # E8519
    current = os.environ
    current["SIGNING_KEY"] = key  # E8519: an alias of os.environ
    os.environ["PATH"] = key  # OK: not a secret
    os.environ.get("API_KEY")  # OK: a read
