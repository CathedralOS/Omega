#!/usr/bin/env python3
"""Exercise actual package-manager installation and upgrade on disposable CI hosts."""

import argparse
from functools import partial
import hashlib
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import shutil
import sys
from threading import Thread

import channels
import github_release as release
import package


def release_input(tag, host, assets, extracted, *, download):
    version = release.version_from_tag(tag)
    commit = release.command("git", "rev-parse", f"refs/tags/{tag}^{{commit}}")
    if download:
        release.download_assets(tag, "CathedralOS/Omega", host, assets)
    row = next(row for row in release.MATRIX if row["host"] == host)
    archive = assets / release.archive_name(version, row)
    bundle = package.verify_archive(archive, extracted, version=version, commit=commit, host=host)
    return archive, package.verify_manifest(bundle)


def installed_identity(executable, manifest, workspace):
    expected = f"omega {manifest['version']} ({manifest['commit']}; {manifest['host']})"
    if package.run([executable, "--version"], cwd=workspace).strip() != expected:
        raise ValueError("package manager's registered command has the wrong version")
    installed = package.verify_manifest(executable.resolve().parent.parent)
    if installed != manifest:
        raise ValueError("installed bundle differs from the verified release asset")


def rebuild_existing_project(executable, workspace):
    environment = os.environ.copy()
    for name in ("OMEGA_LIBRARY_ROOT", "GH_TOKEN", "GITHUB_TOKEN"):
        environment.pop(name, None)
    home = workspace / "git smoke home"
    environment.update(HOME=str(home), USERPROFILE=str(home),
                       LOCALAPPDATA=str(home), XDG_CACHE_HOME=str(home))
    project = workspace / "git source smoke"
    lock = project / "omega.lock"
    original = lock.read_bytes()
    output = workspace / "upgraded native output"
    package.run([executable, "--offline", "--build-dir", output, project / "main.omg"],
                cwd=workspace, environment=environment, timeout=600)
    program = output / ("omega-program.exe" if os.name == "nt" else "omega-program")
    package.run([program], cwd=workspace, environment=environment, expected=42, timeout=600)
    if lock.read_bytes() != original:
        raise ValueError("compiler upgrade rewrote the accepted dependency lock")
    return hashlib.sha256(original).hexdigest()


def homebrew_upgrade(previous, current, base_url, workspace):
    # A local tap exercises the published formula without publishing a channel.
    # Only the download transport changes; the archive bytes and hashes do not.
    tap = "omega-install-test/releases"
    formula_name = f"{tap}/omega"
    environment = dict(os.environ, HOMEBREW_NO_AUTO_UPDATE="1", HOMEBREW_NO_INSTALL_CLEANUP="1")
    # This fixture edits the formula directly, without tap commits or fetches.
    # Homebrew strips Git identity environment overrides; no Git repo is needed.
    package.run(["brew", "tap-new", "--no-git", tap], environment=environment)
    tap_root = Path(package.run(["brew", "--repository", tap], environment=environment).strip())
    executable = Path(package.run(["brew", "--prefix"], environment=environment).strip()) / "bin/omega"
    try:
        for operation, (archive, manifest) in zip(("install", "upgrade"), (previous, current)):
            archives = {row["host"]: (release.archive_name(manifest["version"], row), "0" * 64)
                        for row in release.MATRIX}
            archives[manifest["host"]] = (archive.name, hashlib.sha256(archive.read_bytes()).hexdigest())
            formula = channels.homebrew_formula(manifest["version"], archives)
            formula = formula.replace(f"{channels.REPOSITORY}/releases/download/v{manifest['version']}", base_url)
            (tap_root / "Formula/omega.rb").write_bytes(formula.encode("utf-8"))
            package.run(["brew", operation, formula_name], environment=environment, timeout=1200)
            installed_identity(executable, manifest, workspace)
            package.run(["brew", "test", formula_name], environment=environment, timeout=600)
            if operation == "install":
                package.git_source_smoke(executable, manifest, workspace)
            else:
                return rebuild_existing_project(executable, workspace)
    finally:
        # This entrypoint refuses non-CI hosts and existing omega commands.
        package.run(["brew", "uninstall", "--force", formula_name], environment=environment)
        package.run(["brew", "untap", tap], environment=environment)


def winget_upgrade(previous, current, base_url, workspace):
    executable = Path(os.environ["LOCALAPPDATA"]) / "Microsoft/WinGet/Links/omega.exe"
    installed_manifest = None
    try:
        for operation, (archive, manifest) in zip(("install", "upgrade"), (previous, current)):
            archives = {manifest["host"]: (archive.name, hashlib.sha256(archive.read_bytes()).hexdigest())}
            document = channels.winget_manifest(manifest["version"], archives)
            document = document.replace(f"{channels.REPOSITORY}/releases/download/v{manifest['version']}", base_url)
            path = workspace / f"{operation}.yaml"
            path.write_bytes(document.encode("utf-8"))
            package.run(["winget", "validate", "--manifest", path])
            package.run(["winget", operation, "--manifest", path, "--scope", "user",
                         "--accept-source-agreements", "--accept-package-agreements", "--disable-interactivity"],
                        timeout=1200)
            installed_manifest = path
            installed_identity(executable, manifest, workspace)
            if operation == "install":
                package.git_source_smoke(executable, manifest, workspace)
            else:
                return rebuild_existing_project(executable, workspace)
    finally:
        if installed_manifest is not None:
            package.run(["winget", "uninstall", "--manifest", installed_manifest,
                         "--accept-source-agreements", "--disable-interactivity"], timeout=600)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("operation", choices=("download", "check"))
    parser.add_argument("--manager", choices=("homebrew", "winget"), required=True)
    parser.add_argument("--host", choices=tuple(package.HOST_TARGETS), required=True)
    parser.add_argument("--previous-tag", required=True)
    parser.add_argument("--tag", required=True)
    parser.add_argument("--workspace", type=Path, default=package.ROOT / "build/install-upgrade")
    arguments = parser.parse_args()
    if arguments.previous_tag == arguments.tag:
        raise ValueError("upgrade needs two distinct releases")
    if arguments.operation == "check" and os.environ.get("GITHUB_ACTIONS") != "true":
        raise ValueError("installer mutation is restricted to disposable GitHub Actions hosts")
    if arguments.operation == "check" and shutil.which("omega"):
        raise ValueError("refusing to replace an existing omega installation")
    host = arguments.host
    if (arguments.manager == "winget") != host.endswith("windows-msvc"):
        raise ValueError("package manager does not match the requested host")
    workspace = arguments.workspace.resolve()
    assets = workspace / "assets"
    inputs = [release_input(tag, host, assets, workspace / f"{arguments.operation}-{index}",
                            download=arguments.operation == "download")
              for index, tag in enumerate((arguments.previous_tag, arguments.tag))]
    if arguments.operation == "download":
        return
    # Serving already authenticated assets on loopback avoids exposing draft
    # credentials to package managers or the compiler being tested.
    handler = partial(SimpleHTTPRequestHandler, directory=str(assets))
    with ThreadingHTTPServer(("127.0.0.1", 0), handler) as server:
        thread = Thread(target=server.serve_forever, daemon=True)
        thread.start()
        try:
            check = winget_upgrade if arguments.manager == "winget" else homebrew_upgrade
            digest = check(*inputs, f"http://127.0.0.1:{server.server_port}", workspace)
        finally:
            server.shutdown()
            thread.join()
    report = dict(manager=arguments.manager, host=host, previous=inputs[0][1]["version"],
                  current=inputs[1][1]["version"], unchanged_lock_sha256=digest,
                  native_exit_before=42, native_exit_after=42)
    (workspace / "result.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(report))


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, RuntimeError) as error:
        print(f"install-upgrade: {error}", file=sys.stderr)
        sys.exit(1)
