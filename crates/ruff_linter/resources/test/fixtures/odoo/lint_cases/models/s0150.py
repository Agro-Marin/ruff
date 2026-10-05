import httpx
import urllib3
import aiohttp
import boto3
timeout = httpx.Timeout(10)
httpx.post(url, timeout=timeout)
httpx.AsyncClient(timeout=timeout)
urllib3.PoolManager(timeout=30)
aiohttp.ClientSession()
boto3.Session(profile_name="x")
