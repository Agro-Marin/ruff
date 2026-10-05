import smtplib
import websocket
import xmlrpc.client
import paramiko
import urllib.request
from paho.mqtt import client as mqtt
from pymodbus.client import ModbusTcpClient, ModbusSerialClient
smtplib.SMTP_SSL(host, 465)
websocket.WebSocketApp(url)
xmlrpc.client.ServerProxy(url)
urllib.request.urlopen(url)
paramiko.SSHClient()
mqtt.Client()
ModbusTcpClient(host)
ModbusSerialClient("/dev/ttyUSB0")
