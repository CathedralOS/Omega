"""Explicit-file recovery bundles, independent of any approved backup transport.

No extraction, upload, credential discovery, or recursive workspace collection.
Library identity/receipt acquisition remains the current Library adapter's job.
"""

import gzip
import hashlib
import io
import json
from pathlib import Path, PurePosixPath, PureWindowsPath
import tarfile

from recovery_state import RecoveryError

MAX_BYTES = 64 * 1024 * 1024
MAX_FILES = 128


def digest(data):
    return hashlib.sha256(data).hexdigest()


def member_name(name):
    path = PurePosixPath(name)
    if (not name or name == "." or "\\" in name or path.is_absolute()
            or PureWindowsPath(name).drive or ".." in path.parts
            or str(path) != name):
        raise RecoveryError("unsafe artifact member")
    return name


def pack(root, paths, destination, base, checkpoint):
    """Caller reviews the exact allowlist for secrets before invoking this."""
    root = Path(root).resolve()
    if not paths or len(paths) > MAX_FILES or len(set(paths)) != len(paths):
        raise RecoveryError("bundle requires a bounded unique explicit file list")
    payloads = {}
    for name in paths:
        member_name(name)
        if name == "manifest.json" or any(
            part.lower() in {".git", ".aws", ".ssh", ".env", "credentials", "auth.json"}
            for part in PurePosixPath(name).parts
        ):
            raise RecoveryError("reserved or credential path in artifact allowlist")
        path = root / name
        if (not path.is_file() or path.is_symlink()
                or root not in path.resolve().parents
                or any(p.is_symlink() for p in path.parents if p != root and root in p.parents)):
            raise RecoveryError("artifact must be a regular file within the declared root")
        if path.stat().st_size > MAX_BYTES:
            raise RecoveryError("artifact exceeds byte budget")
        payloads[name] = path.read_bytes()
        if sum(map(len, payloads.values())) > MAX_BYTES:
            raise RecoveryError("artifact exceeds byte budget")
    manifest = dict(schema=1, status="incomplete-unvalidated", base=base,
                    checkpoint=checkpoint, files={name: dict(sha256=digest(data), bytes=len(data))
                                                 for name, data in sorted(payloads.items())})
    payloads["manifest.json"] = json.dumps(manifest, sort_keys=True).encode()
    # Exclusive output: a duplicate pack cannot overwrite previously uploaded bytes.
    with Path(destination).open("xb") as raw:
        with gzip.GzipFile(fileobj=raw, mode="wb", filename="", mtime=0) as compressed:
            with tarfile.open(fileobj=compressed, mode="w", format=tarfile.USTAR_FORMAT) as archive:
                for name, data in sorted(payloads.items()):
                    info = tarfile.TarInfo(name)
                    info.size, info.mode = len(data), 0o600
                    archive.addfile(info, io.BytesIO(data))
    return verify(destination)


def verify(path, expected=None):
    """Bound decompression and refuse partial, duplicate, unsafe or extra entries."""
    path = Path(path)
    try:
        if path.stat().st_size > MAX_BYTES:
            raise RecoveryError("compressed artifact exceeds byte budget")
        raw = path.read_bytes()
        sha = digest(raw)
        if expected is not None and sha != expected:
            raise RecoveryError("archive checksum mismatch")
        # Consume the gzip trailer as well as tar's early end marker. Otherwise
        # a truncated gzip stream can contain a complete-looking tar prefix.
        tar_budget = MAX_BYTES + (MAX_FILES + 3) * 1024 + 20480
        with gzip.GzipFile(fileobj=io.BytesIO(raw), mode="rb") as compressed:
            expanded = compressed.read(tar_budget + 1)
        if len(expanded) > tar_budget:
            raise RecoveryError("expanded artifact exceeds byte budget")
        payloads, total = {}, 0
        with tarfile.open(fileobj=io.BytesIO(expanded), mode="r|") as archive:
            for member in archive:
                name = member_name(member.name)
                total += member.size
                if (not member.isfile() or name in payloads or member.size < 0
                        or total > MAX_BYTES or len(payloads) >= MAX_FILES + 1):
                    raise RecoveryError("invalid or oversized artifact members")
                payloads[name] = archive.extractfile(member).read()
        manifest = json.loads(payloads.pop("manifest.json"))
        if (manifest.get("schema") != 1 or not manifest.get("base")
                or not manifest.get("checkpoint") or not manifest.get("files")
                or set(manifest["files"]) != set(payloads)):
            raise RecoveryError("incomplete artifact manifest")
        for name, data in payloads.items():
            if manifest["files"][name] != {"sha256": digest(data), "bytes": len(data)}:
                raise RecoveryError("artifact member checksum mismatch")
        return dict(sha256=sha, bytes=len(raw), base=manifest["base"],
                    checkpoint=manifest["checkpoint"], files=len(payloads))
    except (OSError, ValueError, KeyError, TypeError, tarfile.TarError, EOFError) as error:
        raise RecoveryError("unreadable or incomplete recovery artifact") from error
