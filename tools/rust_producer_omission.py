#!/usr/bin/env python3
"""Audit whether a produced closure's declared dependency set omits the
Rust producer (`omega-rust/`).

Contract (omega-rust/README.md): a toolchain, release, or bootstrap input
set omits the Rust producer when its closed dependency set contains no
`omega-rust/` artifact, no `omega-rust/` build step, and no checkout-derived
path — including a `source/library/` location derived from the compiler
checkout rather than declared as a first-class input.

The audit is over a *declared* input set: one or more member manifests
(plain one-path-per-line files, or `*.sources` closure manifests whose
`member` rows are read for their spellings) plus an optional steps file
(one command per line) describing the set's build/invocation surface. The
declarations are trusted as the complete closed set; the tool decides only
whether that set contains the producer — it does not discover transitive
dependencies the manifest forgot to name.

Checks:

- member spellings are canonical relative POSIX paths (same byte rules as
  `tools/bootstrap/source_closure.py`) and resolve inside `--root`;
- no member has an `omega-rust` path component (producer artifact);
- no step token invokes a Rust toolchain binary (`cargo`, `rustc`,
  `rustup`, `mbx`, `nextest`/`cargo-nextest`) — inside a produced closure
  such an invocation can only build or run an `omega-rust` crate;
- no member or step token names a checkout-derived path: `CARGO_MANIFEST_DIR`,
  `OUT_DIR`, `..` traversal that escapes the declared root, an absolute
  path, or a `source/library` reference reached through a Rust-checkout
  derivation (`.../omega-rust/../source/library`, `CARGO_MANIFEST_DIR` joins).

Exit status: 0 when the set omits the producer, 1 when findings prove the
producer is present, 2 on malformed input. `--record OUT.json` writes a
ReleaseRecordSubstrateV1 audit record (subject, checks, findings, result)
for the release-record family; `--require omitted|present` additionally
verifies the verdict matches the caller's expectation.

Portable: Python standard library only; identical invocation on Windows
PowerShell and POSIX shells.
"""

import argparse
import json
import re
import shlex
import sys
from pathlib import Path, PurePosixPath


SCHEMA = "ReleaseRecordSubstrateV1"
SOURCE_HEADER_MEMBER = re.compile(r"^member ")
SOURCE_CLOSURE_HEADER = re.compile(r"^[A-Za-z]+SourceClosureV[0-9]+$")
FORBIDDEN_MEMBER_COMPONENT = "omega-rust"
RUST_TOOLCHAIN_NAMES = {"cargo", "rustc", "rustup", "mbx", "nextest", "cargo-nextest"}
CHECKOUT_DERIVED_MARKERS = ("CARGO_MANIFEST_DIR", "OUT_DIR")


def fail(message: str) -> None:
    raise SystemExit(f"rust producer omission: {message}")


def find(step: str, member, detail: str) -> dict:
    return {"check": step, "member": member, "detail": detail}


def check_spelling(spelling: str, member_label: str, findings: list):
    """Return the canonical relative spelling, recording findings."""
    if any(not 33 <= ord(byte) <= 126 for byte in spelling) or "\\" in spelling or ":" in spelling:
        findings.append(find("canonical-spelling", member_label, f"noncanonical spelling: {spelling}"))
        return None
    relative = PurePosixPath(spelling)
    if str(relative) != spelling or relative.is_absolute() or not relative.parts:
        findings.append(find("canonical-spelling", member_label, f"noncanonical spelling: {spelling}"))
        return None
    if any(part in ("", ".", "..") for part in relative.parts):
        findings.append(find(
            "checkout-derived-path",
            member_label,
            f"path escapes the declared root via traversal: {spelling}",
        ))
        return None
    return relative


def check_member(relative: PurePosixPath, member_label: str, findings: list) -> None:
    """One member must not be an omega-rust artifact or a checkout-derived path."""
    if FORBIDDEN_MEMBER_COMPONENT in relative.parts:
        findings.append(find(
            "producer-artifact",
            member_label,
            f"member is inside `{FORBIDDEN_MEMBER_COMPONENT}/`: {relative}",
        ))


def check_step_token(token: str, step_label: str, findings: list) -> None:
    """One step token must not invoke the Rust toolchain or derive a checkout path."""
    if not token:
        return
    name = token.rsplit("/", 1)[-1]
    if name in RUST_TOOLCHAIN_NAMES:
        findings.append(find(
            "producer-build-step",
            step_label,
            f"step invokes Rust toolchain `{name}`: {token}",
        ))
    if FORBIDDEN_MEMBER_COMPONENT in token:
        findings.append(find(
            "producer-build-step",
            step_label,
            f"step references `{FORBIDDEN_MEMBER_COMPONENT}/`: {token}",
        ))
    if any(marker in token for marker in CHECKOUT_DERIVED_MARKERS):
        findings.append(find(
            "checkout-derived-path",
            step_label,
            f"step derives a path from the Rust checkout: {token}",
        ))
    if ".." in token and "source/library" in token:
        findings.append(find(
            "checkout-derived-path",
            step_label,
            f"step reaches `source/library` by checkout-relative traversal: {token}",
        ))


def iter_manifest_members(path: Path) -> tuple[list[str], int]:
    """Yield member spellings from a manifest.

    A `*.sources` closure manifest contributes its `member` row spellings;
    any other file contributes one non-comment line per member.
    Returns (spellings, line_count).
    """
    try:
        text = path.read_text(encoding="ascii")
    except (OSError, UnicodeDecodeError) as error:
        fail(f"cannot read manifest {path}: {error}")
    lines = text.splitlines()
    closure = bool(lines) and SOURCE_CLOSURE_HEADER.fullmatch(lines[0].strip()) is not None
    spellings = []
    for line_number, line in enumerate(lines, 1):
        stripped = line.strip()
        if line_number == 1 and closure:
            continue
        if not stripped or stripped.startswith("#"):
            continue
        if closure:
            if not SOURCE_HEADER_MEMBER.match(stripped):
                fail(f"non-member row in closure manifest at {path}:{line_number}")
            fields = stripped.split(" ")
            if len(fields) != 5 or any(not field for field in fields):
                fail(f"malformed member row at {path}:{line_number}")
            spellings.append(fields[4])
        else:
            spellings.append(stripped)
    return spellings, len(lines)


def iter_step_lines(path: Path) -> list[tuple[int, str]]:
    try:
        text = path.read_text(encoding="utf-8")
    except OSError as error:
        fail(f"cannot read steps file {path}: {error}")
    lines = []
    for line_number, line in enumerate(text.splitlines(), 1):
        stripped = line.strip()
        if stripped and not stripped.startswith("#"):
            lines.append((line_number, stripped))
    return lines


def audit(manifests: list[Path], steps_files: list[Path], root: Path) -> dict:
    findings: list[dict] = []
    member_count = 0
    step_count = 0
    for manifest_path in manifests:
        if manifest_path.is_symlink() or not manifest_path.is_file():
            fail(f"manifest is not a regular file: {manifest_path}")
        spellings, _ = iter_manifest_members(manifest_path)
        member_count += len(spellings)
        for spelling in spellings:
            relative = check_spelling(spelling, f"{manifest_path.name}:{spelling}", findings)
            if relative is None:
                continue
            resolved = manifest_path.parent.joinpath(*relative.parts)
            try:
                resolved.relative_to(root)
            except ValueError:
                findings.append(find(
                    "checkout-derived-path",
                    spelling,
                    f"member resolves outside the declared root: {spelling}",
                ))
                continue
            check_member(relative, spelling, findings)
    for steps_path in steps_files:
        if steps_path.is_symlink() or not steps_path.is_file():
            fail(f"steps file is not a regular file: {steps_path}")
        for line_number, line in iter_step_lines(steps_path):
            step_count += 1
            try:
                tokens = shlex.split(line, posix=True)
            except ValueError:
                tokens = line.split()
            for token in tokens:
                check_step_token(token, f"{steps_path.name}:{line_number}", findings)
    result = "omitted" if not findings else "present"
    return {
        "schema": SCHEMA,
        "subject": {
            "manifests": [str(path) for path in manifests],
            "steps": [str(path) for path in steps_files],
            "root": str(root),
        },
        "member_count": member_count,
        "step_count": step_count,
        "checks": ["canonical-spelling", "producer-artifact", "producer-build-step", "checkout-derived-path"],
        "findings": findings,
        "result": result,
    }


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Audit a produced closure's declared dependency set for "
        "Rust-producer (`omega-rust/`) presence per the omega-rust/README omission contract."
    )
    parser.add_argument(
        "--manifest",
        action="append",
        default=[],
        metavar="PATH",
        help="member manifest: one relative path per line, or a *.sources closure manifest",
    )
    parser.add_argument(
        "--steps",
        action="append",
        default=[],
        metavar="PATH",
        help="declared build/invocation steps, one command per line",
    )
    parser.add_argument(
        "--root",
        metavar="DIR",
        help="checkout root every member must resolve inside (default: repository root containing the manifests)",
    )
    parser.add_argument("--record", metavar="OUT.json", help="write the audit record")
    parser.add_argument(
        "--require",
        choices=("omitted", "present"),
        help="exit 1 when the verdict does not match the expectation",
    )
    args = parser.parse_args()
    if not args.manifest and not args.steps:
        fail("nothing to audit: supply at least one --manifest or --steps")
    root = Path(args.root).resolve() if args.root else Path(__file__).resolve().parents[1]
    if not root.is_dir():
        fail(f"declared root is not a directory: {root}")
    record = audit(
        [Path(path).resolve() for path in args.manifest],
        [Path(path).resolve() for path in args.steps],
        root,
    )
    if args.record:
        out = Path(args.record)
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_text(json.dumps(record, indent=2) + "\n", encoding="utf-8")
    print(
        f"rust producer omission: {record['result']} "
        f"({record['member_count']} members, {record['step_count']} steps, "
        f"{len(record['findings'])} findings)"
    )
    for finding in record["findings"]:
        print(f"  {finding['check']}: {finding['detail']}", file=sys.stderr)
    if args.require:
        return 0 if record["result"] == args.require else 1
    return 0 if record["result"] == "omitted" else 1


if __name__ == "__main__":
    sys.exit(main())
