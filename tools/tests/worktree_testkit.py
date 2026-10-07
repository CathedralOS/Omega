"""Shared fixtures for the worktree_status test suite: module loading, an
isolated git environment, a seeded repository, and captured main() runs."""

import contextlib
import importlib.util
import io
import os
from pathlib import Path
import subprocess
import sys
import tempfile

STATUS_MODULE = Path(__file__).resolve().parents[1] / "swarm" / "worktree_status.py"


def load_module(path, name):
    specification = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(specification)
    previous = sys.dont_write_bytecode
    try:
        sys.dont_write_bytecode = True
        specification.loader.exec_module(module)
    finally:
        sys.dont_write_bytecode = previous
    return module


def load_status_module():
    return load_module(STATUS_MODULE, "worktree_status_under_test")


def isolated_git_environment():
    environment = {key: value for key, value in os.environ.items()
                   if not key.startswith("GIT_")}
    environment.update(GIT_CONFIG_NOSYSTEM="1",
                       GIT_CONFIG_GLOBAL=os.devnull,
                       GIT_TERMINAL_PROMPT="0")
    return environment


def git(case, directory, *arguments):
    result = subprocess.run(["git", "-C", str(directory), *arguments],
                            capture_output=True, text=True, encoding="utf-8",
                            env=case.environment, timeout=30)
    case.assertEqual(result.returncode, 0, result.stderr)
    return result.stdout.strip()


def seed_repository(case, prefix):
    """One temporary repository on main, identity configured, one commit."""
    temporary = tempfile.TemporaryDirectory(prefix=prefix)
    case.addCleanup(temporary.cleanup)
    root = Path(temporary.name)
    repository = root / "repo"
    repository.mkdir()
    case.environment = isolated_git_environment()
    git(case, repository, "init", "-b", "main")
    for name, value in (("user.name", "Worktree Test"),
                        ("user.email", "test@example.invalid"),
                        ("commit.gpgSign", "false")):
        git(case, repository, "config", name, value)
    (repository / "file.txt").write_text("initial\n", encoding="utf-8")
    git(case, repository, "add", "-A")
    git(case, repository, "commit", "-m", "initial")
    return root, repository


def run_main(module, repository, *arguments):
    output = io.StringIO()
    with contextlib.redirect_stdout(output):
        code = module.main([
            "--repository", str(repository),
            "--base", "main", "--offline", *arguments])
    return code, output.getvalue()
