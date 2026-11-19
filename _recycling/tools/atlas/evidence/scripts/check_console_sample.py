"""Check the pinned Console inputs and output, or replay a verified compiler."""

import argparse
import hashlib
import io
import json
import os
from pathlib import Path
import subprocess
import tempfile
import zipfile


ROOT = Path(__file__).resolve().parents[4]
RECORD = Path(__file__).resolve().parents[1] / "checkpoints/public-console.json"
SAMPLE = "samples/cli/basics/cli_mvp"


def git(*args):
    return subprocess.check_output(["git", *args], cwd=ROOT)


def check(condition, message):
    if not condition:
        raise ValueError(message)


def main():
    record = json.loads(RECORD.read_text())
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("lane", choices=record["runs"])
    parser.add_argument("--compiler", type=Path, help="Check this compiler's SHA-256")
    parser.add_argument("--log", type=Path, help="Compare the original combined output")
    parser.add_argument("--run", action="store_true", help="Run the compiler on a fresh copy of the inputs")
    args = parser.parse_args()
    if args.run and not args.compiler:
        parser.error("--run requires --compiler")
    run = record["runs"][args.lane]
    revision = record["input_revision"]
    for path, expected in (("source/library", record["library_tree"]), (SAMPLE, record["sample_tree"])):
        for source in (revision, run["compiler_revision"]):
            check(git("rev-parse", f"{source}:{path}").decode().strip() == expected, f"changed input: {source}:{path}")
    output = record["combined_output"].encode()
    check(hashlib.sha256(output).hexdigest() == record["combined_output_sha256"], "changed recorded output")
    if args.log:
        check(args.log.read_bytes() == output, "different process output")
    if args.compiler:
        with args.compiler.open("rb") as binary:
            sha = hashlib.sha256()
            for chunk in iter(lambda: binary.read(1024 * 1024), b""):
                sha.update(chunk)
        check(sha.hexdigest() == run["compiler_sha256"], "different compiler")
    if args.run:
        with tempfile.TemporaryDirectory() as temp:
            with zipfile.ZipFile(io.BytesIO(git("archive", "--format=zip", revision, "source/library", SAMPLE))) as files:
                files.extractall(temp)
            env = dict(os.environ, OMEGA_LIBRARY_ROOT=str(Path(temp) / "source/library"), RAYON_NUM_THREADS="1")
            result = subprocess.run([str(args.compiler.resolve()), *record["argv"]], cwd=temp, env=env,
                                    input=bytes.fromhex(record["stdin_hex"]), stdout=subprocess.PIPE,
                                    stderr=subprocess.STDOUT)
            check(result.returncode == record["cli_exit"] and result.stdout == output, "Console run differs")
    print(f"{args.lane}: verified recorded inputs and output" + (", replay passed" if args.run else ""))


if __name__ == "__main__":
    main()
