"""Check an Omega compiler fixture through the public execution engines."""

import subprocess
import tempfile
from pathlib import Path


def check_silent_execution(cli, project, *, entry="build.omg", expected="return"):
    """Require normal return or a declared Trap and empty streams on both engines."""
    if expected not in ("return", "Trap"):
        raise ValueError(f"unknown expected termination {expected!r}")
    with tempfile.TemporaryDirectory(prefix="omega-fixture-output-") as directory:
        for engine in ("native", "psi"):
            output = Path(directory) / engine
            subprocess.run(
                [cli, "run", "--engine", engine, "--outputs", output, entry],
                cwd=project, stdin=subprocess.DEVNULL, check=True,
            )
            result = (output / "termination.tsv").read_text().splitlines()
            fields = result[0].split("\t") if len(result) == 1 else []
            if engine == "psi":
                wanted = ["psi", "return"] if expected == "return" else ["psi", "crash", "Trap"]
                matches = fields == wanted
            elif len(fields) != 5 or fields[0] != "native" or fields[4] != "process-exit-excluded":
                matches = False
            elif expected == "return":
                matches = fields[1:3] == ["exit", "0"]
            else:
                matches = fields[3] == "trap"
            if not matches:
                raise ValueError(f"{engine}: expected {expected}, observed {result!r}")
            for stream in ("stdout", "stderr"):
                data = (output / stream).read_bytes()
                if data:
                    raise ValueError(
                        f"{engine}: expected empty {stream}, observed {data[:64]!r} ({len(data)} bytes)"
                    )
