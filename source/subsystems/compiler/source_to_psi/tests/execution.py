"""Check an Omega compiler fixture through the public execution engines."""

import subprocess
import tempfile
from pathlib import Path


def check_silent_return(cli, project):
    """Both engines must return normally and produce no guest output."""
    with tempfile.TemporaryDirectory(prefix="omega-fixture-output-") as directory:
        for engine in ("native", "psi"):
            output = Path(directory) / engine
            subprocess.run(
                [cli, "run", "--engine", engine, "--outputs", output, "build.omg"],
                cwd=project, stdin=subprocess.DEVNULL, check=True,
            )
            result = (output / "termination.tsv").read_text().splitlines()
            fields = result[0].split("\t") if len(result) == 1 else []
            if engine == "native":
                returned = (
                    len(fields) == 5 and fields[:3] == ["native", "exit", "0"]
                    and fields[4] == "process-exit-excluded"
                )
            else:
                returned = fields == ["psi", "return"]
            if not returned:
                raise ValueError(f"{engine}: expected normal return, observed {result!r}")
            for stream in ("stdout", "stderr"):
                data = (output / stream).read_bytes()
                if data:
                    raise ValueError(
                        f"{engine}: expected empty {stream}, observed {data[:64]!r} ({len(data)} bytes)"
                    )
