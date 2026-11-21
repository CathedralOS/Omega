"""Keep the exact compiler inputs named by a review replay."""

from hashlib import sha256
from pathlib import Path
from tempfile import NamedTemporaryFile


def retain_replay_source(directory: Path, arm: str, data: bytes) -> Path:
    """Different trials can start at case zero in the same worker directory."""
    if arm not in ("control", "changed"):
        raise ValueError(f"unknown replay arm {arm!r}")
    folder = directory / "sources" / ("s_" + sha256(data).hexdigest())
    folder.mkdir(parents=True, exist_ok=True)
    path = folder / f"{arm}.omg"
    if path.exists():
        if path.read_bytes() != data:
            raise ValueError(f"replay source differs from its digest at {path}")
        return path

    # Concurrent replays of identical bytes may safely publish the same name.
    # Close before replacing so a reader can open the completed file on Windows.
    tmp = NamedTemporaryFile(dir=folder, delete=False)
    staged = Path(tmp.name)
    try:
        with tmp:
            tmp.write(data)
        staged.replace(path)
    finally:
        staged.unlink(missing_ok=True)
    return path
