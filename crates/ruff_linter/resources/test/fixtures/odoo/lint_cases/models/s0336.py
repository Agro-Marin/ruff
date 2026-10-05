import subprocess


def f(token):
    subprocess.run(['x'], env={'API_TOKEN': token}, check=True)
