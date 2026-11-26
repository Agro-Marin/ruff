from __future__ import annotations

import logging
import subprocess
import sys
import tempfile
from dataclasses import dataclass
from pathlib import Path

logger = logging.getLogger(__name__)

UV_CONFIG = """\
# Keep mypy and its runtime dependencies compatible when projects pin older versions.
override-dependencies = [
    "mypy==2.4.0",
    "ast-serialize>=0.11.2,<1.0.0",
    "librt>=0.16.0",
]

[extra-build-dependencies]
# The pandas docs theme omits this build dependency.
pydata-sphinx-theme = ["typing-extensions"]
"""


@dataclass(frozen=True, kw_only=True, slots=True)
class Venv:
    project_name: str
    project_path: Path

    @property
    def path(self) -> Path:
        return self.project_path / "venv"

    @property
    def name(self) -> str:
        return self.path.name

    @property
    def python(self) -> Path:
        return self.script("python")

    @property
    def bin(self) -> Path:
        bin_dir = "scripts" if sys.platform == "win32" else "bin"
        return self.path / bin_dir

    def script(self, name: str) -> Path:
        extension = ".exe" if sys.platform == "win32" else ""
        return self.bin / f"{name}{extension}"

    @staticmethod
    def create(*, project: str, parent: Path, python_version: str) -> Venv:
        command = [
            "uv",
            "venv",
            "--quiet",
            "--python",
            python_version,
            "venv",
        ]

        try:
            subprocess.run(
                command, cwd=parent, check=True, capture_output=True, text=True
            )
        except subprocess.CalledProcessError as e:
            msg = f"Failed to create venv for {project}:\n\n{e.stderr}"
            raise RuntimeError(msg) from e

        return Venv(project_name=project, project_path=parent)

    def install(self, pip_install_args: list[str]) -> None:
        logger.debug(f"Installing dependencies: {', '.join(pip_install_args)}")
        command = [
            "uv",
            "pip",
            "install",
            "--python",
            self.python.as_posix(),
            "--quiet",
            "--exclude-newer",
            "2026-10-24T19:50:00Z",
            "mypy",
        ]

        try:
            with tempfile.TemporaryDirectory() as temp_dir:
                config = Path(temp_dir) / "uv.toml"
                config.write_text(UV_CONFIG, encoding="utf-8")
                subprocess.run(
                    [*command, "--config-file", str(config), *pip_install_args],
                    cwd=self.project_path,
                    check=True,
                    capture_output=True,
                    text=True,
                )
        except subprocess.CalledProcessError as e:
            msg = (
                f"Failed to install dependencies for {self.project_name}:\n\n{e.stderr}"
            )
            raise RuntimeError(msg) from e
