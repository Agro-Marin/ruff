# /// script
# requires-python = ">=3.12"
# dependencies = ["tqdm"]
#
# [tool.uv]
# no-build = true
# exclude-newer = "P7D"
# require-build-hashes = true
# preview-features = ["build-dependency-hashes", "missing-exclude-newer-package-lock"]
# ///

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path
from subprocess import CalledProcessError
from typing import NamedTuple

from tqdm import tqdm


class Repository(NamedTuple):
    org: str
    repo: str
    ref: str | None


def main() -> None:
    ruff_args = sys.argv[1:]
    checkouts = Path("checkouts")
    out_dir = Path("ecosystem_all_results")
    github_search_json = Path("github_search.jsonl")
    ruff = Path.cwd().joinpath("ruff")

    out_dir.mkdir(parents=True, exist_ok=True)

    repositories = []
    for line in github_search_json.read_text().splitlines():
        item = json.loads(line)
        if item["path"] != "pyproject.toml":
            continue
        repositories.append(
            Repository(
                item["owner"],
                item["repo"],
                item.get("ref"),
            ),
        )

    successes = 0
    errors = 0
    for repository in tqdm(repositories):
        project_dir = checkouts.joinpath(f"{repository.org}:{repository.repo}")
        if not project_dir.is_dir():
            tqdm.write(f"Missing {project_dir}")
            errors += 1
            continue

        try:
            output = subprocess.run(
                [ruff, *ruff_args, "."],
                cwd=project_dir,
                capture_output=True,
                text=True,
                check=False,
            )
        except CalledProcessError as e:
            tqdm.write(f"Ruff failed on {project_dir}: {e}")
            errors += 1
            continue

        org_repo = f"{repository.org}:{repository.repo}"
        out_dir.joinpath(f"{org_repo}.stdout.txt").write_text(output.stdout)
        out_dir.joinpath(f"{org_repo}.stderr.txt").write_text(output.stderr)
        successes += 1
    print(f"Success: {successes} Error {errors}")


if __name__ == "__main__":
    main()
