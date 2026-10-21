#!/usr/bin/env python3
"""Check retained execution products through the public CLI on the native host."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--omega", type=Path, required=True)
    parser.add_argument("--repository", type=Path, default=Path(__file__).resolve().parents[3])
    options = parser.parse_args()
    repository = options.repository.resolve()
    omega = options.omega.resolve()
    environment = {**os.environ, "OMEGA_LIBRARY_ROOT": str(repository / "source/library")}

    def invoke(*arguments):
        return subprocess.run([str(omega), *map(str, arguments)], env=environment,
                              stdin=subprocess.DEVNULL, capture_output=True, timeout=120)

    # Reuse a real runnable fact, not a second language fixture.
    source = repository / "samples/cli/facts/correlated_invariant_store/parameter"
    with tempfile.TemporaryDirectory(prefix="omega-keep-check-") as workspace:
        project = Path(workspace) / "project"
        temporary = Path(workspace) / "retained"
        project.mkdir()
        temporary.mkdir()
        environment.update(TMPDIR=str(temporary), TMP=str(temporary), TEMP=str(temporary))
        for name in ("main.omg", "build.omg"):
            shutil.copy2(source / name, project / name)
        review = invoke("update", "--project", project, "--dangerously-accept-all")
        assert review.returncode == 0, review.stderr.decode(errors="replace")
        run = invoke("run", "--both", "--keep", project / "main.omg")
        text = run.stderr.decode(errors="replace")
        assert run.returncode == 0 and "native exit 0\n" in text and "interp exit: 0\n" in text, text
        matches = re.findall(r"^build dir kept: (.+)$", text, re.MULTILINE)
        assert len(matches) == 1, text
        kept = Path(matches[0]).resolve()
        # Cleanup owns only this invocation's explicitly reported temporary directory.
        assert kept.parent == temporary.resolve()
        assert kept.name.startswith("omega-probe-")
        try:
            assert not (kept / "native.object.txt").exists(), "retention eagerly formats the native evidence graph"
            image = kept / ("omega-program.exe" if os.name == "nt" else "omega-program")
            terminal = kept / "program.oterm"
            assert image.is_file() and image.stat().st_size > 0
            assert terminal.read_bytes().startswith(b"PSIART\0\0")
            assert (kept / "native.identity.txt").stat().st_size > 0
            for name in ("terminal.stdout", "terminal.stderr", "terminal.effects"):
                assert (kept / name).is_file(), name
            replay = subprocess.run([str(image)], stdin=subprocess.DEVNULL,
                                    capture_output=True, timeout=30)
            assert replay.returncode == 0, replay
            retained_bytes = sum(path.stat().st_size for path in kept.rglob("*") if path.is_file())
            assert retained_bytes < 10 * 1024 * 1024, retained_bytes
            print(json.dumps(dict(native_exit=run.returncode, terminal_exit=0,
                                  retained_native_exit=replay.returncode, retained_bytes=retained_bytes,
                                  native_sha256=hashlib.sha256(image.read_bytes()).hexdigest(),
                                  terminal_sha256=hashlib.sha256(terminal.read_bytes()).hexdigest())))
        finally:
            shutil.rmtree(kept)


if __name__ == "__main__":
    main()
