"""Check a fixture with the interpreter and its independently compiled host executable."""

import os
import subprocess
import tempfile
from pathlib import Path


def compile_fixture(cli, project, entry, directory):
    """Return the published executable path and process evidence for one host build."""
    report = directory / "compile-report.txt"
    subprocess.run(
        [cli, "--report-file", report, "--build-dir", directory / "build", entry],
        cwd=project, stdin=subprocess.DEVNULL, check=True,
    )
    lines = report.read_text(encoding="utf-8").splitlines()
    prefix = "published native output to "
    paths = [Path(line.removeprefix(prefix)) for line in lines if line.startswith(prefix)]
    if len(paths) != 1:
        raise ValueError(f"expected one published executable in {report}, got {paths}")
    executable = project / paths[0]
    if not executable.is_file():
        raise FileNotFoundError(executable)
    return executable, lines


def trap_status(report):
    for line in report:
        if line.startswith("trap-presentation: signal "):
            return -int(line.split()[-1])
        if line.startswith("trap-presentation: exit "):
            status = int(line.split()[-1])
            return status & 0xffffffff if os.name == "nt" else status
    raise ValueError("compiled fixture has no known Trap presentation on this host")


def check_silent_execution(cli, project, *, entry="build.omg", expected="return"):
    """Require normal return or a declared Trap and empty streams on both paths."""
    if expected not in ("return", "Trap"):
        raise ValueError(f"unknown expected termination {expected!r}")
    with tempfile.TemporaryDirectory(prefix="omega-fixture-output-") as directory:
        root = Path(directory)
        executable, lines = compile_fixture(cli, project, entry, root)
        if "process-exit-excluded" not in lines:
            raise ValueError("compiled fixture must exclude process exit")
        native = subprocess.run([executable], cwd=project, stdin=subprocess.DEVNULL, capture_output=True)
        status = 0 if expected == "return" else trap_status(lines)
        if (native.returncode, native.stdout, native.stderr) != (status, b"", b""):
            raise ValueError(f"unexpected host result for {entry}: {native!r}")

        output = root / "psi"
        subprocess.run(
            [cli, "run", "--outputs", output, entry],
            cwd=project, stdin=subprocess.DEVNULL, check=True,
        )
        wanted = "psi\treturn" if expected == "return" else "psi\tcrash\tTrap"
        result = (output / "termination.tsv").read_text().splitlines()
        if result != [wanted]:
            raise ValueError(f"expected {wanted!r}, observed {result!r}")
        for stream in ("stdout", "stderr"):
            data = (output / stream).read_bytes()
            if data:
                raise ValueError(f"expected empty {stream}, observed {data[:64]!r} ({len(data)} bytes)")
