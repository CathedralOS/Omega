#!/usr/bin/env python3
"""Coordinate the fixed GitHub release matrix; packaging stays in package.py."""

import argparse
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile

import package

MATRIX = [
    {"runner": "windows-2022", "host": "x86_64-pc-windows-msvc", "suffix": ".zip"},
    {"runner": "macos-15", "host": "aarch64-apple-darwin", "suffix": ".tar.gz"},
    {"runner": "ubuntu-22.04", "host": "x86_64-unknown-linux-gnu", "suffix": ".tar.gz"},
    {"runner": "ubuntu-22.04-arm", "host": "aarch64-unknown-linux-gnu", "suffix": ".tar.gz"},
]


def version_from_tag(tag):
    if not re.fullmatch(r"v(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)-(alpha|beta|rc)\.(0|[1-9][0-9]*)", tag):
        raise ValueError("expected a prerelease tag such as v0.1.0-alpha.1")
    return tag[1:]


def command(*arguments):
    return package.run(arguments).strip()


def api(path, *arguments):
    return json.loads(command("gh", "api", path, *arguments))


def existing_release(repository, tag):
    # Listing distinguishes a missing release from permission/network errors;
    # a failed `gh release view` must not be treated as permission to create.
    pages = json.loads(command("gh", "api", "--paginate", "--slurp",
                               f"repos/{repository}/releases?per_page=100"))
    return next((release for page in pages for release in page if release["tag_name"] == tag), None)


def verify_tag(tag, commit):
    result = subprocess.run(["git", "show-ref", "--verify", "--quiet", f"refs/tags/{tag}"],
                            cwd=package.ROOT)
    if result.returncode == 1:
        return False
    if result.returncode != 0:
        raise RuntimeError("could not inspect release tag")
    if command("git", "rev-parse", f"refs/tags/{tag}^{{commit}}") != commit:
        raise ValueError("existing tag identifies a different source commit; choose a new version")
    return True


def prepare(tag, repository):
    version = version_from_tag(tag)
    commit = command("git", "rev-parse", "HEAD")
    command("git", "merge-base", "--is-ancestor", commit, "origin/main")
    verify_tag(tag, commit)
    if existing_release(repository, tag):
        raise ValueError("release already exists; assets are never overwritten; choose a new version")
    values = dict(tag=tag, version=version, commit=commit, matrix=json.dumps({"include": MATRIX}))
    with Path(os.environ["GITHUB_OUTPUT"]).open("a", encoding="utf-8", newline="\n") as output:
        for name, value in values.items():
            output.write(f"{name}={value}\n")


def archive_name(version, row):
    return f"omega-{version}-{row['host']}{row['suffix']}"


def checked_assets(directory, version, commit):
    expected = [directory / archive_name(version, row) for row in MATRIX]
    expected += [archive.with_name(archive.name + ".sha256") for archive in expected.copy()]
    if {path.name for path in directory.iterdir()} != {path.name for path in expected}:
        raise ValueError("release assets must contain exactly all four bundles and their checksums")
    for row in MATRIX:
        with tempfile.TemporaryDirectory(prefix="omega asset check ") as temporary:
            package.verify_archive(directory / archive_name(version, row), Path(temporary),
                                   version=version, commit=commit, host=row["host"])
    return expected


def notes(commit, verified):
    run = f"https://github.com/{os.environ['GITHUB_REPOSITORY']}/actions/runs/{os.environ['GITHUB_RUN_ID']}"
    state = ("All four freshly downloaded bundles passed source checking, local package review, "
             "native compilation and execution with an empty PATH and fresh user cache. "
             "A separate cold-cache Git dependency test passed acquisition, review, and offline native execution using host Git."
             if verified else "Download validation is pending. Do not publish this draft until the linked workflow succeeds.")
    return (f"Experimental Omega compiler prerelease from `{commit}`.\n\n{state}\n\n"
            f"[Release workflow]({run}). This smoke coverage is not a compiler-completeness claim.\n\n"
            "Extract the entire archive and add its `bin` directory to PATH. Keep the executable "
            "and `lib/omega/library` together. Rust is not required for local compilation; Git-source "
            "acquisition requires Git. `omega update` updates project packages, not this installation.\n\n"
            "Test hosts: Windows Server 2022 x64, macOS 14 ARM64, Ubuntu 22.04 x64/ARM64. "
            "No broader minimum-OS compatibility or platform code signing is claimed.\n")


def write_release_notes(commit, verified):
    path = package.ROOT / "build/release-notes.md"
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(notes(commit, verified), encoding="utf-8")
    return path


def publish(tag, repository, commit, directory):
    version = version_from_tag(tag)
    assets = checked_assets(directory, version, commit)
    if existing_release(repository, tag):
        raise ValueError("release already exists; refusing to replace its assets")
    command("git", "fetch", "origin", "main", "--tags")
    command("git", "merge-base", "--is-ancestor", commit, "origin/main")
    if not verify_tag(tag, commit):
        api(f"repos/{repository}/git/refs", "--method", "POST", "-f", f"ref=refs/tags/{tag}", "-f", f"sha={commit}")
    command("gh", "release", "create", tag, *map(str, assets), "--repo", repository,
            "--verify-tag", "--draft", "--prerelease", "--title", f"Omega {version}",
            "--notes-file", str(write_release_notes(commit, False)))


def download_assets(tag, repository, host, directory):
    row = next(row for row in MATRIX if row["host"] == host)
    name = archive_name(version_from_tag(tag), row)
    directory.mkdir(parents=True, exist_ok=True)
    command("gh", "release", "download", tag, "--repo", repository,
            "--pattern", name, "--pattern", name + ".sha256", "--dir", str(directory))


def verify_download(tag, commit, host, directory):
    version = version_from_tag(tag)
    row = next(row for row in MATRIX if row["host"] == host)
    with tempfile.TemporaryDirectory(prefix="omega downloaded release ") as temporary:
        workspace = Path(temporary)
        name = archive_name(version, row)
        bundle = package.verify_archive(directory / name, workspace,
                                        version=version, commit=commit, host=host)
        package.smoke_test(bundle, workspace, git_sources=True)
        package.verify_manifest(bundle)


def finish(tag, repository, commit):
    release = existing_release(repository, tag)
    if not release or not release["draft"] or not release["prerelease"]:
        raise ValueError("expected the draft prerelease created by this run")
    if not verify_tag(tag, commit):
        raise ValueError("release tag is missing")
    command("gh", "release", "edit", tag, "--repo", repository,
            "--notes-file", str(write_release_notes(commit, True)))
    print(release["html_url"])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("operation", choices=("prepare", "publish", "download", "verify", "finish"))
    parser.add_argument("--assets", type=Path, default=package.ROOT / "build/release-assets")
    arguments = parser.parse_args()
    tag = os.environ["RELEASE_TAG"]
    repository = os.environ["GITHUB_REPOSITORY"]
    version_from_tag(tag)
    if arguments.operation == "prepare":
        prepare(tag, repository)
    else:
        commit = os.environ["RELEASE_COMMIT"]
        if not re.fullmatch(r"[0-9a-f]{40}", commit):
            raise ValueError("expected an exact source commit")
        if arguments.operation == "publish":
            publish(tag, repository, commit, arguments.assets)
        elif arguments.operation == "download":
            download_assets(tag, repository, os.environ["RELEASE_HOST"], arguments.assets)
        elif arguments.operation == "verify":
            verify_download(tag, commit, os.environ["RELEASE_HOST"], arguments.assets)
        else:
            finish(tag, repository, commit)


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, RuntimeError, subprocess.TimeoutExpired) as error:
        print(f"release: {error}", file=sys.stderr)
        sys.exit(1)
