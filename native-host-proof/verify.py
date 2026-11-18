"""Verify and execute the two committed public fixture images on their matching host."""
import hashlib
import json
import os
from pathlib import Path
import platform
import stat
import subprocess

HERE = Path(__file__).resolve().parent
TARGETS = {
    "linux_x86_64": ("Linux", "x86_64", "X64"),
    "linux_arm64": ("Linux", "aarch64", "ARM64"),
    "macos_arm64": ("Darwin", "arm64", "ARM64"),
    "windows_x86_64": ("Windows", "amd64", "X64"),
}
FIXTURE = "tests/fixtures/run/providers/target_selected_implementation"


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def verify_format(target, data):
    if target.startswith("linux_"):
        require(data[:7] == b"\x7fELF\x02\x01\x01", "expected little-endian ELF64")
        require(int.from_bytes(data[16:18], "little") in (2, 3), "expected executable ELF")
        cpu = int.from_bytes(data[18:20], "little")
        require(cpu == (62 if target == "linux_x86_64" else 183), "ELF target mismatch")
        return "ELF64"
    if target == "macos_arm64":
        require(data[:8] == bytes.fromhex("cffaedfe0c000001"), "expected arm64 Mach-O")
        require(int.from_bytes(data[12:16], "little") == 2, "expected executable Mach-O")
        return "Mach-O 64"
    header = int.from_bytes(data[60:64], "little")
    require(data[:2] == b"MZ" and 64 <= header <= len(data)-26, "invalid DOS header")
    require(data[header:header+6] == b"PE\x00\x00\x64\x86", "expected AMD64 PE")
    require(data[header+24:header+26] == bytes.fromhex("0b02"), "expected PE32+")
    return "PE32+"


def main():
    manifest = json.loads((HERE / "images.json").read_text())
    target = os.environ["OMEGA_TARGET"]
    host = (platform.system(), platform.machine().lower(), os.environ["RUNNER_ARCH"])
    require(TARGETS[target] == host, f"runner does not match {target}: {host}")
    print(f"host {host} kernel {platform.release()}", flush=True)
    for name in ("build.omg", "main.omg"):
        source = subprocess.check_output(["git", "show", f"HEAD:{FIXTURE}/{name}"])
        require(hashlib.sha256(source).hexdigest() == manifest["sources_sha256"][name],
                f"fixture source digest changed: {name}")
    records = []
    for lane in ("candidate", "baseline"):
        filename = "omega-program.exe" if target == "windows_x86_64" else "omega-program"
        path = HERE / "images" / lane / target / filename
        require(path.is_file() and not path.is_symlink(), f"missing image {path}")
        contents = path.read_bytes()
        digest = hashlib.sha256(contents).hexdigest()
        require(digest == manifest["images"][target][lane], f"{lane} image digest mismatch")
        kind = verify_format(target, contents)
        if os.name != "nt":
            path.chmod(path.stat().st_mode | stat.S_IXUSR)
        process = subprocess.run([str(path)], cwd=path.parent, stdin=subprocess.DEVNULL, timeout=30)
        row = dict(target=target, host=host, lane=lane, sha256=digest, size=len(contents),
                   format=kind, exit=process.returncode)
        print(json.dumps(row, sort_keys=True), flush=True)
        records.append(row)
    require(all(row["exit"] == 0 for row in records), "fixture returned a nonzero exit")


if __name__ == "__main__":
    main()
