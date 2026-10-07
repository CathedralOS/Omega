"""Shared TYPESAFE_API_KEY discovery and request plumbing for the advisor tools."""


import json
import os
import subprocess
from pathlib import Path
import urllib.error
import urllib.request


def read_key(path):
    if not path.exists():
        return ""
    keys = [line.partition("=")[2].strip().strip("\"'")
            for line in path.read_text(encoding="utf-8-sig").splitlines()
            if line.partition("=")[0].strip() == "TYPESAFE_API_KEY"]
    return keys[0] if len(keys) == 1 else ""


def main_checkout_root():
    """Linked worktrees share the main checkout's .git dir; its parent is the
    checkout root where local-only files like build/typesafe.env.txt live."""
    try:
        done = subprocess.run(
            ["git", "rev-parse", "--path-format=absolute", "--git-common-dir"],
            capture_output=True, text=True, timeout=15)
    except (OSError, subprocess.SubprocessError):
        return None
    if done.returncode != 0:
        return None
    return Path(done.stdout.strip()).parent


def find_key(key_file):
    key = os.environ.get("TYPESAFE_API_KEY", "").strip()
    if key:
        return key
    candidates = ([key_file] if key_file else [])
    candidates.append(Path("build/typesafe.env.txt"))
    root = main_checkout_root()
    if root is not None:
        candidates.append(root / "build/typesafe.env.txt")
    candidates.append(Path.home() / ".config" / "typesafe" / "typesafe.env.txt")
    for path in candidates:
        key = read_key(path)
        if key:
            return key
    return ""


def post(endpoint, key, request, timeout=60):
    """POST one advisory JSON request; the parsed response, or None on
    transport failure. A response echoing the credential is refused."""
    http = urllib.request.Request(
        endpoint, data=json.dumps(request).encode(),
        headers={"Authorization": f"Bearer {key}",
                 "Content-Type": "application/json"}, method="POST")
    try:
        with urllib.request.build_opener(
                urllib.request.HTTPRedirectHandler()).open(
                http, timeout=timeout) as connection:
            response = json.load(connection)
    except (urllib.error.URLError, TimeoutError, urllib.error.HTTPError):
        return None
    if key in json.dumps(response):
        raise RuntimeError("Response contains the credential; refusing output")
    return response
