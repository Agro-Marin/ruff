ok = consteq(record.access_token, token)
sig = hmac.compare_digest(received_signature, expected)
key = consteq(provider.oauth_token, header_value)
