import re
PATTERN = re.compile(r'\w+')
def process(self, nodes):
    for node in nodes:
        if re.search(r'pattern', node.text):
            pass
        if PATTERN.search(node.text):
            pass
        if some_regex.search(node.text):
            pass
