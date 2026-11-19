#!/usr/bin/env python3
"""Build the Darwin ARM64 C registrar used to call the reconstructed handler."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--cc", default=os.environ.get("CC") or "cc")
    arguments = parser.parse_args()
    if platform.system() != "Darwin" or platform.machine().lower() not in ("arm64", "aarch64"):
        parser.error("this fixture requires a Darwin ARM64 host")
    fixture = Path(__file__).resolve().parent
    output = (arguments.output or fixture.parents[5] / "build/descriptor-callback-provider").resolve()
    output.mkdir(parents=True, exist_ok=True)
    provider = fixture / "provider.c"
    library = output / "libomega_descriptor_fixture.dylib"
    command = [arguments.cc, "-std=c11", "-Wall", "-Wextra", "-Werror", "-O2",
               "-arch", "arm64", "-dynamiclib", "-Wl,-install_name," + library.name,
               str(provider), "-o", str(library)]
    result = subprocess.run(command, capture_output=True, text=True, timeout=60)
    if result.returncode:
        raise SystemExit(result.stdout + result.stderr or
                         f"provider build failed with exit {result.returncode}: {command}")
    print(json.dumps({
        "host": platform.platform(),
        "library": str(library),
        "library_sha256": hashlib.sha256(library.read_bytes()).hexdigest(),
        "provider_sha256": hashlib.sha256(provider.read_bytes()).hexdigest(),
        "recipe_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        "commands": [command],
        "build_exit": result.returncode,
    }, indent=2))


if __name__ == "__main__":
    main()
