"""Collect redistribution notices from the resolved compiler build inputs."""

import json
from pathlib import Path
import shutil
import subprocess

HERE = Path(__file__).resolve().parent
# New license expressions require distribution review rather than silently
# inheriting a permissive policy from a crate's name. This is not a legal audit.
REVIEWED_LICENSES = {
    "MIT", "Apache-2.0", "MIT OR Apache-2.0", "Apache-2.0 OR MIT",
    "MIT/Apache-2.0", "Unlicense OR MIT", "MIT OR Unlicense",
    "(MIT OR Apache-2.0) AND Unicode-3.0",
    "Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT",
    "Apache-2.0 WITH LLVM-exception", "BSD-3-Clause",
}


def dependency_closure(metadata):
    packages = {package["id"]: package for package in metadata["packages"]}
    nodes = {node["id"]: node for node in metadata["resolve"]["nodes"]}
    pending = [package["id"] for package in packages.values()
               if package["name"] == "omega" and package["source"] is None]
    if len(pending) != 1:
        raise ValueError("expected exactly one workspace omega package")
    seen = set()
    while pending:
        identity = pending.pop()
        if identity in seen:
            continue
        seen.add(identity)
        pending.extend(dependency["pkg"] for dependency in nodes[identity]["deps"]
                       if any(kind["kind"] in (None, "build") for kind in dependency["dep_kinds"]))
    return sorted((packages[identity] for identity in seen if packages[identity]["source"]),
                  key=lambda package: (package["name"], package["version"], package["id"]))


def license_files(package):
    root = Path(package["manifest_path"]).parent
    files = [path for path in root.rglob("*") if path.is_file()
             and path.name.upper().split(".")[0].split("-")[0] in ("LICENSE", "LICENCE", "COPYING", "NOTICE", "COPYRIGHT")]
    if package.get("license_file"):
        files.append(root / package["license_file"])
    if files:
        return [(path.relative_to(root), path) for path in sorted(set(files))]
    # This published crate omits its upstream license. The copy is pinned to
    # the exact source commit recorded in its .cargo_vcs_info.json.
    if (package["name"], package["version"]) == ("iced-x86", "1.21.0"):
        return [(Path("LICENSE.txt"), HERE / "licenses/iced-x86-1.21.0.txt")]
    raise ValueError(f"no license text found for {package['id']}")


def stage_dependency_notices(destination, metadata, sysroot):
    destination.mkdir(parents=True)
    inventory = []
    for package in dependency_closure(metadata):
        if package["license"] not in REVIEWED_LICENSES:
            raise ValueError(f"unreviewed license for {package['id']}: {package['license']}")
        name = f"{package['name']}-{package['version']}"
        target = destination / name
        target.mkdir()  # Same name/version from distinct sources needs explicit review.
        copied = []
        for relative, source in license_files(package):
            output = target / relative
            output.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(source, output)
            copied.append(output.relative_to(destination).as_posix())
        inventory.append(dict(name=package["name"], version=package["version"],
                              source=package["source"], license=package["license"],
                              repository=package.get("repository"), files=copied))
    rust = Path(sysroot) / "share/doc/rust"
    target = destination / "rust"
    target.mkdir()
    shutil.copyfile(rust / "COPYRIGHT-library.html", target / "COPYRIGHT-library.html")
    shutil.copytree(rust / "licenses", target / "licenses")
    (destination / "dependencies.json").write_text(
        json.dumps(inventory, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def collect(destination, repository, host):
    metadata = json.loads(subprocess.check_output(
        ["cargo", "metadata", "--locked", "--format-version", "1", "--filter-platform", host],
        cwd=repository, text=True, encoding="utf-8"))
    sysroot = subprocess.check_output(["rustc", "--print", "sysroot"],
                                     cwd=repository, text=True, encoding="utf-8").strip()
    stage_dependency_notices(destination, metadata, sysroot)

