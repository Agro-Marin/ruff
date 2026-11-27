# /// script
# requires-python = ">=3.12"
# dependencies = []
#
# [tool.uv]
# no-build = true
# exclude-newer = "P7D"
# require-build-hashes = true
# preview-features = ["build-dependency-hashes", "missing-exclude-newer-package-lock"]
# ///

from __future__ import annotations

import argparse
from pathlib import Path

URL = "https://user-images.githubusercontent.com/1309177/{}.svg"
URL_LIGHT = URL.format("232603516-4fb4892d-585c-4b20-b810-3db9161831e4")
URL_DARK = URL.format("232603514-c95e9b0f-6b31-43de-9a80-9e844173fd6a")

GITHUB = f"""
<p align="center">
  <picture align="center">
    <source media="(prefers-color-scheme: dark)" srcset="{URL_DARK}">
    <source media="(prefers-color-scheme: light)" srcset="{URL_LIGHT}">
    <img alt="Shows a bar chart with benchmark results." src="{URL_LIGHT}">
  </picture>
</p>
"""

PYPI = f"""
<p align="center">
  <img alt="Shows a bar chart with benchmark results." src="{URL_LIGHT}">
</p>
"""

MK_DOCS = f"""
<p align="center">
  <img alt="Shows a bar chart with benchmark results." src="{URL_LIGHT}#only-light">
</p>

<p align="center">
  <img alt="Shows a bar chart with benchmark results." src="{URL_DARK}#only-dark">
</p>
"""


def main(target: str) -> None:
    """Modify the README.md to support the given target."""
    with Path("README.md").open(encoding="utf8") as fp:
        content = fp.read()
        if GITHUB not in content:
            msg = "README.md is not in the expected format."
            raise ValueError(msg)

    if target == "pypi":
        with Path("README.md").open("w", encoding="utf8") as fp:
            fp.write(content.replace(GITHUB, PYPI))
    elif target == "mkdocs":
        with Path("README.md").open("w", encoding="utf8") as fp:
            fp.write(content.replace(GITHUB, MK_DOCS))
    else:
        msg = f"Unknown target: {target}"
        raise ValueError(msg)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(
        description="Modify the README.md to support a specific deployment target.",
    )
    parser.add_argument(
        "--target",
        type=str,
        required=True,
        choices=("pypi", "mkdocs"),
    )
    args = parser.parse_args()

    main(target=args.target)
