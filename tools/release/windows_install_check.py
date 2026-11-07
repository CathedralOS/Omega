#!/usr/bin/env python3
"""Build and exercise MSI upgrades only on disposable GitHub Windows runners."""

import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess

import github_release as release
import install_upgrade
import package
import windows_installer as installer


def user_path():
    import winreg
    with winreg.OpenKey(winreg.HKEY_CURRENT_USER, "Environment") as key:
        try:
            return winreg.QueryValueEx(key, "Path")[0]
        except FileNotFoundError:
            return ""


def msiexec(operation, product, log, *, expected=0):
    result = subprocess.run(["msiexec.exe", operation, str(product), "/qn", "/norestart", "/l*v", str(log)],
                            timeout=600, check=False)
    if result.returncode != expected:
        raise ValueError(f"msiexec {operation} returned {result.returncode}, expected {expected}; see {log}")


def check(workspace, previous, current):
    installed = Path(os.environ["LOCALAPPDATA"]) / "Programs/Omega"
    if installed.exists() or shutil.which("omega"):
        raise ValueError("installer test refuses an existing Omega installation")
    before = user_path()
    executable = installed / "bin/omega.exe"
    active = None
    try:
        for index, (artifact, manifest, version) in enumerate((previous, current)):
            # Record before invoking, so cleanup also covers a successful install
            # followed by a verification failure.
            msiexec("/i", artifact, workspace / f"install-{index}.log")
            active = installer.product_code(version)
            install_upgrade.installed_identity(executable, manifest, workspace)
            if str(executable.parent).casefold() not in user_path().casefold().split(";"):
                raise ValueError("installer did not register the per-user bin directory")
            if index == 0:
                package.git_source_smoke(executable, manifest, workspace)
            else:
                lock_digest = install_upgrade.rebuild_existing_project(executable, workspace)
        # Windows Installer error 1603 is required; accepting the old package
        # would silently replace a newer compiler with an older release.
        msiexec("/i", previous[0], workspace / "downgrade.log", expected=1603)
        install_upgrade.installed_identity(executable, current[1], workspace)
    finally:
        if active:
            msiexec("/x", "{" + active + "}", workspace / "uninstall.log")
    if installed.exists():
        raise ValueError("uninstall left the compiler bundle behind")
    if user_path().strip(";") != before.strip(";"):
        raise ValueError("uninstall changed unrelated user PATH entries")
    if not (workspace / "git source smoke/omega.lock").is_file() or not (workspace / "git smoke home").is_dir():
        raise ValueError("uninstall removed the project or user cache")
    (workspace / "result.json").write_text(json.dumps(dict(
        previous=previous[1], current=current[1], lock_sha256=lock_digest,
        install=True, upgrade=True, downgrade_rejected=True, uninstall=True,
        path_restored=True, native_exit=42), indent=2) + "\n", encoding="utf-8")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("phase", choices=("download", "check"))
    parser.add_argument("--previous-tag", required=True)
    parser.add_argument("--tag", required=True)
    parser.add_argument("--previous-installer-version", required=True)
    parser.add_argument("--installer-version", required=True)
    parser.add_argument("--wix", type=Path, default=Path("build/wix/wix.exe"))
    parser.add_argument("--workspace", type=Path, default=Path("build/windows-installer"))
    options = parser.parse_args()
    if os.name != "nt" or os.environ.get("GITHUB_ACTIONS") != "true":
        raise ValueError("installation mutations require a disposable GitHub Windows runner")
    versions = [installer.installer_version(value) for value in
                (options.previous_installer_version, options.installer_version)]
    if tuple(map(int, versions[0].split("."))) >= tuple(map(int, versions[1].split("."))):
        raise ValueError("the new installer version must increase")
    workspace = options.workspace.resolve()
    workspace.mkdir(parents=True, exist_ok=True)
    products = []
    for role, tag, version in zip(("previous", "current"), (options.previous_tag, options.tag), versions):
        assets = workspace / role / "assets"
        if options.phase == "download":
            release.download_assets(tag, "CathedralOS/Omega", installer.HOST, assets)
        else:
            archive, manifest = install_upgrade.release_input(tag, installer.HOST, assets,
                                                              workspace / role / "extracted", download=False)
            bundle = workspace / role / "extracted" / archive.stem
            artifact = installer.build(bundle, version, workspace / role / "installer", options.wix.resolve())
            products.append((artifact, manifest, version))
    if options.phase == "check":
        check(workspace, *products)


if __name__ == "__main__":
    main()
