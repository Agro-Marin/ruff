if hmac.new(key, body, sha256).hexdigest() != received:
    pass
def verify(key, body, received):
    expected = hmac.digest(key, body, 'sha256')
    return expected == received
def checksum(data, published):
    digest = hashlib.sha256(data).hexdigest()
    return digest == published
