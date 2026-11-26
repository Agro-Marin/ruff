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

"""Update ruff.json in schemastore.

This script will clone `astral-sh/schemastore`, update the schema and push the changes
to a new branch tagged with the ruff git hash. You should see a URL to create the PR
to schemastore in the CLI.

Usage:

    uv run --script scripts/update_schemastore.py
"""

from __future__ import annotations

import enum
import json
from pathlib import Path
from subprocess import check_call, check_output
from tempfile import TemporaryDirectory
from typing import NamedTuple, assert_never

RUFF_REPO = "https://github.com/astral-sh/ruff"

RUFF_ROOT = Path(__file__).parent.parent

RUFF_SCHEMA = RUFF_ROOT / "ruff.schema.json"

RUFF_JSON = Path("schemas/json/ruff.json")


class SchemastoreRepos(NamedTuple):
    fork: str
    upstream: str


class GitProtocol(enum.Enum):
    SSH = "ssh"
    HTTPS = "https"

    def schemastore_repos(self) -> SchemastoreRepos:
        match self:
            case GitProtocol.SSH:
                return SchemastoreRepos(
                    fork="git@github.com:astral-sh/schemastore.git",
                    upstream="git@github.com:SchemaStore/schemastore.git",
                )
            case GitProtocol.HTTPS:
                return SchemastoreRepos(
                    fork="https://github.com/astral-sh/schemastore.git",
                    upstream="https://github.com/SchemaStore/schemastore.git",
                )
            case _:
                assert_never(self)


def update_schemastore(
    schemastore_path: Path, schemastore_repos: SchemastoreRepos
) -> None:
    if not (schemastore_path / ".git").is_dir():
        check_call(
            ["git", "clone", schemastore_repos.fork, schemastore_path, "--depth=1"]
        )
        check_call(
            [
                "git",
                "remote",
                "add",
                "upstream",
                schemastore_repos.upstream,
            ],
            cwd=schemastore_path,
        )
    check_call(["git", "fetch", "upstream"], cwd=schemastore_path)
    current_sha = check_output(["git", "rev-parse", "HEAD"], text=True).strip()
    branch = f"update-ruff-{current_sha}"
    check_call(
        ["git", "switch", "-c", branch],
        cwd=schemastore_path,
    )
    check_call(
        ["git", "reset", "--hard", "upstream/master"],
        cwd=schemastore_path,
    )

    src = schemastore_path / "src"
    check_call(["npm", "ci", "--ignore-scripts"], cwd=schemastore_path)

    schema = json.loads(RUFF_SCHEMA.read_text())
    schema["$id"] = "https://json.schemastore.org/ruff.json"
    (src / RUFF_JSON).write_text(
        json.dumps(dict(schema.items()), indent=2, ensure_ascii=False),
    )
    check_call(
        [
            "../node_modules/prettier/bin/prettier.cjs",
            "--plugin",
            "prettier-plugin-sort-json",
            "--write",
            RUFF_JSON,
        ],
        cwd=src,
    )

    if check_output(["git", "status", "-s"], cwd=schemastore_path).strip():
        commit_url = f"{RUFF_REPO}/commit/{current_sha}"
        commit_body = (
            f"This updates ruff's JSON schema to [{current_sha}]({commit_url})"
        )
        check_call(["git", "add", (src / RUFF_JSON).as_posix()], cwd=schemastore_path)
        check_call(
            [
                "git",
                "commit",
                "-m",
                "Update ruff's JSON schema",
                "-m",
                commit_body,
            ],
            cwd=schemastore_path,
        )
        check_call(
            ["git", "push", "--set-upstream", "origin", branch, "--force"],
            cwd=schemastore_path,
        )
    else:
        print("No changes")


def determine_git_protocol(argv: list[str] | None = None) -> GitProtocol:
    import argparse

    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    parser.add_argument(
        "--proto",
        choices=[proto.value for proto in GitProtocol],
        default="https",
        help="Protocol to use for git authentication",
    )
    args = parser.parse_args(argv)
    return GitProtocol(args.proto)


def main() -> None:
    schemastore_repos = determine_git_protocol().schemastore_repos()
    schemastore_existing = RUFF_ROOT / "schemastore"
    if schemastore_existing.is_dir():
        update_schemastore(schemastore_existing, schemastore_repos)
    else:
        with TemporaryDirectory(prefix="ruff-schemastore-") as temp_dir:
            update_schemastore(Path(temp_dir), schemastore_repos)


if __name__ == "__main__":
    main()
