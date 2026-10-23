#!/usr/bin/env python3
"""Exercise case-run argument errors and native/Terminal outputs through omega."""
import argparse
import csv
import os
from pathlib import Path
import shutil
import subprocess
import tempfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--omega", type=Path, required=True)
    parser.add_argument("--repository", type=Path, default=Path(__file__).resolve().parents[3])
    options = parser.parse_args()
    repository = options.repository.resolve()
    environment = {**os.environ, "OMEGA_LIBRARY_ROOT": str(repository / "source/library")}
    omega = options.omega.resolve()
    with tempfile.TemporaryDirectory(prefix="omega-run-arguments-") as temporary:
        workspace = Path(temporary)

        def invoke(*arguments):
            return subprocess.run([str(omega), *map(str, arguments)], cwd=workspace,
                                  env=environment, stdin=subprocess.DEVNULL,
                                  capture_output=True, timeout=120)

        # A missing project proves these errors precede package/compiler work.
        rejected = 0
        for flag, noun in (("--input", "file"), ("--inputs", "directory"),
                           ("--outputs", "directory")):
            for value in ("", "--both", "--keep", "--target"):
                companion = (["--input", "missing.stdin"] if flag == "--outputs"
                             else ["--outputs", "out"])
                result = invoke("run", flag, value, *companion, "missing.omg")
                assert result.returncode == 64, (flag, value, result)
                assert f"{flag} requires a {noun}".encode() in result.stderr, result.stderr
                assert b"usage: omega run" in result.stderr and not result.stdout, result
                rejected += 1
            result = invoke("run", "missing.omg", flag)
            assert result.returncode == 64, (flag, result)
            assert f"{flag} requires a {noun}".encode() in result.stderr, result.stderr
            rejected += 1
        assert list(workspace.iterdir()) == [], "argument errors created files"

        # Reuse the same runnable fact as the retained-products CLI regression.
        project = workspace / "project"
        project.mkdir()
        source = repository / "samples/cli/facts/correlated_invariant_store/parameter"
        for name in ("main.omg", "build.omg"):
            shutil.copy2(source / name, project / name)
        review = invoke("update", "--project", project, "--dangerously-accept-all")
        assert review.returncode == 0, review
        inputs = workspace / "case inputs"
        inputs.mkdir()
        for name in ("b.stdin", "a.stdin"):
            (inputs / name).write_bytes(b"case input\n")
        # Option-looking filenames remain usable through an explicit path.
        (workspace / "--both").write_bytes(b"explicit input\n")
        runs = (
            (["--both", "--inputs", inputs], ["a", "b"], True),
            (["--input", inputs / "b.stdin", "--both", "--input", "./--both"],
             ["b", "--both"], True),
            (["--input", inputs / "a.stdin"], ["a"], False),
        )
        for index, (arguments, names, compared) in enumerate(runs):
            outputs = workspace / f"case outputs {index}"
            result = invoke("run", *arguments, "--outputs", outputs, project / "main.omg")
            assert result.returncode == 0, result
            with (outputs / "results.tsv").open(newline="") as stream:
                rows = list(csv.DictReader(stream, delimiter="\t"))
            assert [row["case"] for row in rows] == names, rows
            for row in rows:
                assert row["native"] == "0", row
                assert row["interpreter"] == ("agrees" if compared else "not-requested"), row
                assert row["interpreter_exit"] == ("0" if compared else ""), row
                for suffix in ("stdout", "stderr"):
                    assert (outputs / f"{row['case']}.{suffix}").read_bytes() == b""
                    reference = outputs / f"{row['case']}.terminal.{suffix}"
                    if compared:
                        assert reference.read_bytes() == b""
                    else:
                        assert not reference.exists()
        print(f"{rejected} usage errors; 5 native cases; 4 Terminal cases; exact streams and statuses")


if __name__ == "__main__":
    main()
