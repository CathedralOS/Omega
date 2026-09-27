#!/usr/bin/env python3
"""Build, relocate, test, and archive one host's Omega installation."""

import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import shutil
import subprocess
import stat
import sys
import tarfile
import tempfile
import zipfile

ROOT = Path(__file__).resolve().parents[2]
HOST_TARGETS = {
    "x86_64-pc-windows-msvc": "windows_x86_64",
    "x86_64-unknown-linux-gnu": "linux_x86_64",
    "aarch64-unknown-linux-gnu": "linux_arm64",
    "aarch64-apple-darwin": "macos_arm64",
}


def run(command, *, cwd=ROOT, environment=None, expected=0, timeout=7200, capture=True):
    print("+ " + " ".join(map(str, command)), flush=True)
    result = subprocess.run(list(map(str, command)), cwd=cwd, env=environment,
                            text=True, encoding="utf-8", errors="replace",
                            stdout=subprocess.PIPE if capture else None,
                            stderr=subprocess.STDOUT, timeout=timeout)
    if result.returncode != expected:
        raise RuntimeError(f"command returned {result.returncode}, expected {expected}:\n{result.stdout or ''}")
    return result.stdout or ""


def library_files(root):
    files = []
    for path in root.rglob("*"):
        if path.is_symlink():
            raise ValueError(f"library symbolic links are not permitted: {path}")
        if path.is_file() and path.suffix == ".omg":
            files.append(path.relative_to(root).as_posix())
    if "std/build.omg" not in files or not any(name.startswith("core/") for name in files):
        raise ValueError("incomplete toolchain library")
    return sorted(files)


def library_digest(root):
    digest = hashlib.sha256(b"omega-toolchain-library-v1\0")
    for name in library_files(root):
        encoded = name.encode("utf-8")
        contents = (root / name).read_bytes()
        digest.update(len(encoded).to_bytes(8, "little"))
        digest.update(encoded)
        digest.update(len(contents).to_bytes(8, "little"))
        digest.update(contents)
    return digest.hexdigest()


def stage_bundle(destination, binary, library, metadata):
    destination.mkdir()
    executable = destination / "bin" / binary.name
    executable.parent.mkdir()
    shutil.copy2(binary, executable)
    executable.chmod(0o755)
    for name in library_files(library):
        target = destination / "lib/omega/library" / name
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(library / name, target)
    for name in ("LICENSE-MIT", "LICENSE-APACHE"):
        target = destination / "licenses" / name
        target.parent.mkdir(exist_ok=True)
        shutil.copyfile(ROOT / name, target)
    files = {path.relative_to(destination).as_posix(): hashlib.sha256(path.read_bytes()).hexdigest()
             for path in sorted(destination.rglob("*")) if path.is_file()}
    manifest = dict(metadata, schema="omega-release-bundle/1", files=files)
    (destination / "manifest.json").write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def archive_bundle(bundle, archive):
    if archive.name.endswith(".zip"):
        with zipfile.ZipFile(archive, "w", compression=zipfile.ZIP_DEFLATED) as output:
            for path in sorted(bundle.rglob("*")):
                if path.is_file():
                    output.write(path, path.relative_to(bundle.parent).as_posix())
    else:
        with tarfile.open(archive, "w:gz") as output:
            output.add(bundle, arcname=bundle.name)


def verify_manifest(bundle):
    manifest = json.loads((bundle / "manifest.json").read_text(encoding="utf-8"))
    actual = {path.relative_to(bundle).as_posix(): hashlib.sha256(path.read_bytes()).hexdigest()
              for path in bundle.rglob("*") if path.is_file() and path != bundle / "manifest.json"}
    if actual != manifest["files"]:
        raise ValueError("bundle content differs from manifest")
    if library_digest(bundle / "lib/omega/library") != manifest["library_sha256"]:
        raise ValueError("bundle library digest differs from manifest")
    return manifest


def extract_bundle(archive, destination):
    """Extract the fixed bundle layout, rejecting links and path escapes."""
    if archive.name.endswith(".zip"):
        name = archive.name[:-4]
        reader = zipfile.ZipFile(archive)
        members = [(entry.orig_filename, stat.S_IFMT(entry.external_attr >> 16)
                    in (0, stat.S_IFREG, stat.S_IFDIR)) for entry in reader.infolist()]
    elif archive.name.endswith(".tar.gz"):
        name = archive.name[:-7]
        reader = tarfile.open(archive)
        members = [(entry.name, entry.isfile() or entry.isdir()) for entry in reader.getmembers()]
    else:
        raise ValueError("expected a .zip or .tar.gz bundle")
    with reader:
        seen = set()
        for member, ordinary in members:
            parts = PurePosixPath(member).parts
            if (not ordinary or not parts or parts[0] != name or "\\" in member or "\0" in member
                    or any(part in (".", "..", "") or ":" in part or part.endswith((" ", "."))
                           for part in member.rstrip("/").split("/"))
                    or parts in seen):
                raise ValueError(f"unsafe or duplicate archive member: {member}")
            seen.add(parts)
        reader.extractall(destination)
    return destination / name


def verify_archive(archive, destination, *, version, commit, host):
    expected = f"{hashlib.sha256(archive.read_bytes()).hexdigest()}  {archive.name}"
    checksum = archive.with_name(archive.name + ".sha256").read_text(encoding="ascii").strip()
    if checksum != expected:
        raise ValueError(f"archive checksum mismatch: {archive}")
    bundle = extract_bundle(archive, destination)
    manifest = verify_manifest(bundle)
    if (manifest.get("schema") != "omega-release-bundle/1" or manifest.get("dirty") is not False
            or manifest.get("version") != version or manifest.get("commit") != commit
            or manifest.get("host") != host):
        raise ValueError("bundle identity differs from the requested release")
    return bundle


def publish_archive(archive, destination):
    checksum_path = destination.with_name(destination.name + ".sha256")
    if checksum_path.exists():
        raise ValueError(f"refusing to overwrite {checksum_path}")
    # Exclusive creation prevents a concurrent packager from replacing an
    # existing release. Remove our incomplete output if copying fails.
    with destination.open("xb") as output:
        try:
            with archive.open("rb") as source:
                shutil.copyfileobj(source, output)
        except BaseException:
            output.close()
            destination.unlink()
            raise
    checksum = hashlib.sha256(destination.read_bytes()).hexdigest()
    with checksum_path.open("x", encoding="ascii", newline="\n") as output:
        output.write(f"{checksum}  {destination.name}\n")


def smoke_test(bundle, workspace):
    manifest = verify_manifest(bundle)
    executable = bundle / "bin" / ("omega.exe" if os.name == "nt" else "omega")
    environment = os.environ.copy()
    environment.pop("OMEGA_LIBRARY_ROOT", None)
    # Local compilation must not find Cargo, rustc, a linker, or repository
    # helpers through the producer's PATH. Git-source acquisition is separate.
    environment["PATH"] = ""
    home = workspace / "smoke home"
    home.mkdir(mode=0o700)
    environment.update(HOME=str(home), USERPROFILE=str(home),
                       LOCALAPPDATA=str(home), XDG_CACHE_HOME=str(home))
    expected = f"omega {manifest['version']} ({manifest['commit']}; {manifest['host']})"
    if run([executable, "--version"], cwd=workspace, environment=environment).strip() != expected:
        raise ValueError("executable identity differs from bundle manifest")
    # An unrelated CWD and a library override pointing at absent backing expose
    # accidental fallback to the checkout even while the producer remains there.
    check = workspace / "check.omg"
    check.write_text((ROOT / "tools/release/smoke/check.omg").read_text(encoding="utf-8"), encoding="utf-8")
    run([executable, "--check", check], cwd=workspace, environment=environment, timeout=300)
    invalid = dict(environment, OMEGA_LIBRARY_ROOT=str(workspace / "missing library"))
    result = subprocess.run([str(executable), "--check", str(check)], cwd=workspace,
                            env=invalid, capture_output=True, text=True, timeout=300)
    if result.returncode == 0 or "missing Omega toolchain library" not in result.stderr + result.stdout:
        raise ValueError("invalid explicit library did not reject without fallback")
    # Remove the bundled library temporarily. A release must not find the still
    # accessible producer checkout. Restore before further checks and archiving.
    library = bundle / "lib/omega/library"
    hidden = bundle / "lib/omega/library.hidden"
    library.rename(hidden)
    try:
        result = subprocess.run([str(executable), "--check", str(check)], cwd=workspace,
                                env=environment, capture_output=True, text=True, timeout=300)
        if result.returncode == 0 or "missing Omega toolchain library" not in result.stderr + result.stdout:
            raise ValueError("missing installed library did not reject without checkout fallback")
    finally:
        hidden.rename(library)
    source = next((library / "core").rglob("*.omg"))
    original = source.read_bytes()
    try:
        source.write_bytes(original + b"\n// mismatched release library\n")
        result = subprocess.run([str(executable), "--check", str(check)], cwd=workspace,
                                env=environment, capture_output=True, text=True, timeout=300)
        if result.returncode == 0 or "does not match this compiler" not in result.stderr + result.stdout:
            raise ValueError("mismatched installed library did not reject")
    finally:
        source.write_bytes(original)
    if os.name != "nt":
        shim = workspace / "omega symlink"
        shim.symlink_to(executable)
        run([shim, "--check", check], cwd=workspace, environment=environment, timeout=300)
    project = workspace / "native smoke"
    project.mkdir()
    for name in ("main.omg", "build.omg"):
        text = (ROOT / "tools/release/smoke" / name).read_text(encoding="utf-8")
        text = text.replace("@STDLIB@", json.dumps((library / "std").as_posix()))
        (project / name).write_text(text, encoding="utf-8")
    # This is our fixed local fixture, not a user's dependency graph. Exercise
    # normal review publication rather than bypassing the package policy gate.
    target = HOST_TARGETS[manifest["host"]]
    run([executable, "update", "--project", project, "--target", target, "--offline"],
        cwd=workspace, environment=environment, expected=3, timeout=600)
    review = project / "build/package-manager" / f"review-{target}.txt"
    document = review.read_bytes().decode("utf-8")
    document, accepted = re.subn(r"(?m)^(decision .+) pending$", r"\1 accept", document)
    if not accepted:
        raise ValueError("native smoke fixture produced no pending review decisions")
    review.write_bytes(document.encode("utf-8"))
    run([executable, "update", "--resume", "--project", project, "--offline"],
        cwd=workspace, environment=environment, timeout=600)
    run([executable, "--offline", project / "main.omg"], cwd=workspace, environment=environment, timeout=600)
    run([executable, "run", project / "main.omg"], cwd=workspace, environment=environment, expected=42, timeout=600)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=ROOT / "build/releases")
    parser.add_argument("--version", help="release version; defaults to workspace version")
    parser.add_argument("--allow-dirty", action="store_true", help="produce an explicitly dirty development bundle")
    arguments = parser.parse_args()
    version = arguments.version or re.search(r'^version = "([^"]+)"', (ROOT / "Cargo.toml").read_text(), re.M)[1]
    if not re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?", version):
        raise ValueError("version must be a filesystem-safe semantic version")
    commit = run(["git", "rev-parse", "HEAD"]).strip()
    dirty = bool(run(["git", "status", "--porcelain", "--untracked-files=normal"]).strip())
    if dirty and not arguments.allow_dirty:
        raise ValueError("release packaging requires a clean checkout; --allow-dirty is for development only")
    if dirty:
        version += "-dirty"
    host_info = run(["rustc", "-vV"])
    host = re.search(r"^host: (.+)$", host_info, re.M)[1]
    if host not in HOST_TARGETS:
        raise ValueError(f"release smoke coverage is not defined for host {host}")
    library = ROOT / "source/library"
    digest = library_digest(library)
    environment = dict(os.environ, OMEGA_RELEASE_VERSION=version,
                       OMEGA_RELEASE_COMMIT=commit, OMEGA_RELEASE_LIBRARY_SHA256=digest)
    if host.endswith("windows-msvc"):
        flags = environment.get("CARGO_ENCODED_RUSTFLAGS")
        if flags is None:
            if environment.get("RUSTFLAGS"):
                raise ValueError("use CARGO_ENCODED_RUSTFLAGS rather than RUSTFLAGS for release packaging")
            flags = ""
        environment["CARGO_ENCODED_RUSTFLAGS"] = flags + ("\x1f" if flags else "") + "-Ctarget-feature=+crt-static"
    cargo = shutil.which("mbx") or "cargo"
    run([cargo, "build", "--locked", "--release", "--target", host, "-p", "omega", "--bin", "omega"],
        environment=environment, capture=False)
    target_directory = Path(json.loads(run(["cargo", "metadata", "--no-deps", "--format-version", "1"]))["target_directory"])
    binary = target_directory / host / "release" / ("omega.exe" if os.name == "nt" else "omega")
    if library_digest(library) != digest:
        raise ValueError("library changed while compiler was building")
    if run(["git", "rev-parse", "HEAD"]).strip() != commit:
        raise ValueError("source commit changed while compiler was building")
    if not dirty and run(["git", "status", "--porcelain", "--untracked-files=normal"]).strip():
        raise ValueError("source checkout changed while compiler was building")
    metadata = dict(version=version, commit=commit, host=host, rustc=host_info.strip(),
                    dirty=dirty, library_sha256=digest)
    arguments.output.mkdir(parents=True, exist_ok=True)
    name = f"omega-{version}-{host}"
    suffix = ".zip" if os.name == "nt" else ".tar.gz"
    destination = arguments.output.resolve() / (name + suffix)
    if destination.exists():
        raise ValueError(f"refusing to overwrite {destination}")
    with tempfile.TemporaryDirectory(prefix="omega bundle ") as temporary:
        scratch = Path(temporary)
        bundle = scratch / name
        stage_bundle(bundle, binary, library, metadata)
        archive = scratch / (name + suffix)
        archive_bundle(bundle, archive)
        extracted = scratch / "relocated installation"
        extracted.mkdir()
        extract_bundle(archive, extracted)
        smoke_test(extracted / name, scratch)
        verify_manifest(extracted / name)
        publish_archive(archive, destination)
    print(destination)


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, RuntimeError, subprocess.TimeoutExpired) as error:
        print(f"package: {error}", file=sys.stderr)
        sys.exit(1)
