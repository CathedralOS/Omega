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


def configure(case, directory, name):
    for key, value in (("user.name", name),
                       ("user.email", "test@example.invalid"),
                       ("commit.gpgSign", "false"),
                       ("core.autocrlf", "false")):
        git(case, directory, "config", key, value)


def git_input(case, directory, *arguments, input_text=""):
    result = subprocess.run(["git", "-C", str(directory), *arguments],
                            input=input_text, capture_output=True, text=True,
                            encoding="utf-8", env=case.environment, timeout=30)
    case.assertEqual(result.returncode, 0, result.stderr)
    return result.stdout.strip()


def seed_remote_pair(case, prefix, name):
    """A bare remote and two writer clones on main, identity configured."""
    temporary = tempfile.TemporaryDirectory(prefix=prefix)
    case.addCleanup(temporary.cleanup)
    root = Path(temporary.name)
    remote = root / "remote.git"
    a, b = root / "writer a", root / "writer b"
    case.environment = isolated_git_environment()
    git(case, root, "init", "--bare", str(remote))
    configure(case, remote, name)
    git(case, root, "init", "-b", "main", str(a))
    configure(case, a, name)
    return root, remote, a, b


def clone_remote_pair(case, remote, a, b, name):
    """Push a's main to the bare remote and clone it into b."""
    git(case, a, "remote", "add", "origin", str(remote))
    git(case, a, "push", "origin", "HEAD:refs/heads/main")
    git(case, a.parent, "clone", "--branch", "main", str(remote), str(b))
    configure(case, b, name)


def start_tool(case, tool, directory, *arguments):
    return subprocess.Popen([sys.executable, str(tool), "--repository", str(directory),
                             *arguments], stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                            text=True, encoding="utf-8", env=case.environment)


def finish_tool(case, process, label):
    try:
        output, error = process.communicate(timeout=30)
    except subprocess.TimeoutExpired:
        process.kill()
        process.communicate()
        case.fail(f"Local {label} command timed out")
    return process.returncode, output, error
