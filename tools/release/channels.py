#!/usr/bin/env python3
"""Generate Homebrew and WinGet manifests from the four verified release bundles."""

import argparse
import hashlib
import json
from pathlib import Path
import sys

import github_release as release

REPOSITORY = "https://github.com/CathedralOS/Omega"


def homebrew_formula(version, archives):
    def asset(host):
        name, digest = archives[host]
        return f'url "{REPOSITORY}/releases/download/v{version}/{name}"\n      sha256 "{digest}"'
    return f'''class Omega < Formula
  desc "Experimental proof-carrying systems language compiler"
  homepage "{REPOSITORY}"
  version "{version}"
  license any_of: ["MIT", "Apache-2.0"]

  depends_on "git"

  on_macos do
    depends_on macos: :sonoma
    depends_on arch: :arm64
    {asset("aarch64-apple-darwin")}
  end

  on_linux do
    on_intel do
      {asset("x86_64-unknown-linux-gnu")}
    end
    on_arm do
      {asset("aarch64-unknown-linux-gnu")}
    end
  end

  def install
    libexec.install Dir["*"]
    bin.install_symlink libexec/"bin/omega"
  end

  test do
    assert_match "omega {version} ", shell_output("#{{bin}}/omega --version")
    (testpath/"main.omg").write("machine smoke(value: u32) -> u32 {{ value }}\\n")
    system bin/"omega", "--check", testpath/"main.omg"
  end
end
'''


def winget_manifest(version, archives):
    name, digest = archives["x86_64-pc-windows-msvc"]
    bundle = name.removesuffix(".zip")
    return f'''# yaml-language-server: $schema=https://aka.ms/winget-manifest.singleton.1.10.0.schema.json
PackageIdentifier: CathedralOS.Omega
PackageVersion: {json.dumps(version)}
PackageLocale: en-US
Publisher: CathedralOS
PackageName: Omega
License: MIT OR Apache-2.0
LicenseUrl: {REPOSITORY}/blob/v{version}/LICENSE-MIT
ShortDescription: Experimental proof-carrying systems language compiler
PackageUrl: {REPOSITORY}
Installers:
- Architecture: x64
  InstallerType: zip
  NestedInstallerType: portable
  NestedInstallerFiles:
  - RelativeFilePath: {bundle}/bin/omega.exe
    PortableCommandAlias: omega
  InstallerUrl: {REPOSITORY}/releases/download/v{version}/{name}
  InstallerSha256: {digest.upper()}
  Dependencies:
    PackageDependencies:
    - PackageIdentifier: Git.Git
ManifestType: singleton
ManifestVersion: 1.10.0
'''


def generate(assets, output, tag, commit):
    version = release.version_from_tag(tag)
    release.checked_assets(assets, version, commit)
    archives = {}
    for row in release.MATRIX:
        name = release.archive_name(version, row)
        archives[row["host"]] = (name, hashlib.sha256((assets / name).read_bytes()).hexdigest())
    # Do not overwrite a prepared channel update for another release.
    output.mkdir(parents=True, exist_ok=False)
    formula = output / "Formula"
    formula.mkdir()
    (formula / "omega.rb").write_bytes(homebrew_formula(version, archives).encode("utf-8"))
    winget = output / "winget"
    winget.mkdir()
    (winget / "CathedralOS.Omega.yaml").write_bytes(winget_manifest(version, archives).encode("utf-8"))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--assets", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--tag", required=True)
    parser.add_argument("--commit", required=True)
    arguments = parser.parse_args()
    generate(arguments.assets, arguments.output, arguments.tag, arguments.commit)


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, RuntimeError) as error:
        print(f"channels: {error}", file=sys.stderr)
        sys.exit(1)
