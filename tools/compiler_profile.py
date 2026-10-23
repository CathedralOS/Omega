"""Require an optimized, identified compiler before starting validation.

The loaded binary reports its build-time profile. Runtime Cargo variables and
directory names are not evidence about an already built executable. Historical
corpus runners without this protocol require a fresh Cargo build receipt bound
to their exact bytes; never execute an old runner merely to probe an unknown flag.
"""

import hashlib
import json
import os
from pathlib import Path
import re
import subprocess


SCHEMA = "omega-compiler-build-profile-v1"
CARGO_RECEIPT_SCHEMA = "omega-cargo-effective-profile-v1"
OPTIMIZED = {"1", "2", "3", "s", "z"}


class CompilerProfileError(ValueError):
    pass


def optimized_environment():
    """Use the same optimized profiles for automatic CLI and corpus builds."""
    return dict(os.environ, CARGO_PROFILE_DEV_OPT_LEVEL="1", CARGO_PROFILE_TEST_OPT_LEVEL="1")


def is_optimized_profile(profile):
    return (isinstance(profile, dict) and profile.get("schema") == SCHEMA
            and isinstance(profile.get("opt_level"), str) and profile["opt_level"] in OPTIMIZED)


def effective_optimization(level, flags):
    """Cargo's profile label precedes the actual Rust codegen flag overrides."""
    if not isinstance(flags, list) or not all(isinstance(flag, str) for flag in flags):
        raise CompilerProfileError("compiler fingerprint lacks actual Rust flags")
    if any(option.startswith("@") for option in flags):
        raise CompilerProfileError("cannot verify compiler optimization through a rustflags response file")
    options = iter(flags)
    for option in options:
        if option in {"-C", "--codegen"}:
            codegen = next(options, "")
        elif option.startswith("-C"):
            codegen = option[2:]
        elif option.startswith("--codegen="):
            codegen = option[len("--codegen="):]
        else:
            codegen = ""
        if codegen.startswith("opt-level="):
            level = codegen[len("opt-level="):]
    if not isinstance(level, str) or level not in OPTIMIZED:
        raise CompilerProfileError(f"compiler has unoptimized or unknown effective opt-level ({level})")
    return level


def top_level_executable_fingerprints(binary, target):
    """Identify an unhashed Cargo bin through its exact internal output bytes.

    Cargo reports a top-level executable path rather than its unit output. Both
    Cargo layouts retain that output beside the unit's fingerprint. Target name
    and bytes must select one output; stale identical outputs are ambiguous even
    when their flags agree. A library fingerprint cannot stand in for the bin.
    """
    if target.get("kind") != ["bin"]:
        return set()
    name = target["name"].replace("-", "_")
    suffix = ".exe" if binary.suffix == ".exe" else ""
    digest = hashlib.sha256(binary.read_bytes()).digest()
    outputs = {}
    for filename in {binary.name, name + suffix}:
        for output in (binary.parent / "build").glob("*/*/out/" + filename):
            if output.is_file() and hashlib.sha256(output.read_bytes()).digest() == digest:
                fingerprint = output.parent.parent / "fingerprint" / ("bin-" + name + ".json")
                outputs.setdefault(output, set()).add(fingerprint)
    for output in (binary.parent / "deps").glob(name + "-*" + suffix):
        match = re.fullmatch(re.escape(name) + r"-([0-9a-f]+)" + re.escape(suffix), output.name)
        if (match and output.is_file()
                and hashlib.sha256(output.read_bytes()).digest() == digest):
            outputs.setdefault(output, set())
            fingerprints = (binary.parent / ".fingerprint").glob("*-" + match.group(1))
            for directory in fingerprints:
                outputs.setdefault(output, set()).add(directory / ("bin-" + name + ".json"))
    if len(outputs) != 1:
        return set()
    fingerprints = next(iter(outputs.values()))
    if len(fingerprints) != 1:
        return set()
    fingerprint = next(iter(fingerprints))
    return {fingerprint} if fingerprint.is_file() else set()


def artifact_codegen(item):
    """Find the exact artifact's flags in Cargo or mbx's fingerprint layout.

    Cargo's artifact JSON reports the profile before RUSTFLAGS/config overrides.
    Only its matching fingerprint records those effective flags, including for
    cached builds. An unfamiliar/missing fingerprint refuses rather than guessing.
    """
    name = item["target"]["name"].replace("-", "_")
    candidates = set()
    for filename in item.get("filenames", []):
        artifact = Path(filename)
        if artifact.parent.name == "out":
            directory = artifact.parent.parent / "fingerprint"
            candidates.update(directory.glob("*-" + name + ".json"))
        elif artifact.parent.name == "deps":
            suffix = re.search(r"-([0-9a-f]+)(?:\.[^.]+)?$", artifact.name)
            if suffix:
                fingerprints = artifact.parent.parent / ".fingerprint"
                for directory in fingerprints.glob("*-" + suffix.group(1)):
                    candidates.update(directory.glob("*-" + name + ".json"))
        elif item.get("executable") == str(artifact):
            candidates.update(top_level_executable_fingerprints(artifact, item["target"]))
    if len(candidates) != 1:
        raise CompilerProfileError(f"{name}: cannot identify the exact Cargo compiler fingerprint")
    path = candidates.pop()
    try:
        contents = path.read_bytes()
        fingerprint = json.loads(contents)
    except (OSError, ValueError) as error:
        raise CompilerProfileError(f"{name}: cannot read Cargo compiler fingerprint: {error}") from error
    if not isinstance(fingerprint, dict):
        raise CompilerProfileError(f"{name}: invalid Cargo compiler fingerprint")
    flags = fingerprint.get("rustflags")
    level = effective_optimization(item.get("profile", {}).get("opt_level"), flags)
    return dict(opt_level=level, rustflags=flags,
                fingerprint=dict(path=str(path), sha256=hashlib.sha256(contents).hexdigest(),
                                 contents=contents.decode("utf-8")))


def require_optimized(binary):
    """Inspect immutable producer bytes before any sample or fixture execution."""
    binary = Path(binary).absolute()
    try:
        image = binary.read_bytes()
        identity = hashlib.sha256(image).hexdigest()
        bound_receipt = None
        if SCHEMA.encode() in image:
            result = subprocess.run([str(binary), "--build-profile"], stdin=subprocess.DEVNULL,
                                    capture_output=True, text=True, encoding="utf-8",
                                    errors="replace", timeout=10)
            if result.returncode:
                raise CompilerProfileError(f"{binary}: build-profile query failed ({result.returncode})")
            profile = json.loads(result.stdout)
        else:
            # New gates still need to compare genuinely historical source.
            # Only a freshly captured Cargo artifact profile can identify it.
            receipt_path = Path(str(binary) + ".compiler-profile.json")
            if not receipt_path.is_file():
                raise CompilerProfileError(f"{binary}: unknown compiler build profile (no metadata or bound Cargo receipt)")
            receipt_bytes = receipt_path.read_bytes()
            receipt = json.loads(receipt_bytes)
            if not isinstance(receipt, dict) or receipt.get("binary_sha256") != identity:
                raise CompilerProfileError(f"{binary}: compiler profile receipt has a different binary digest")
            if receipt.get("schema") != CARGO_RECEIPT_SCHEMA:
                raise CompilerProfileError(f"{binary}: unknown Cargo effective profile receipt schema")
            profile = receipt["profile"]
            cargo_profiles = receipt.get("cargo_profiles")
            codegen = receipt.get("effective_codegen")
            if (not isinstance(cargo_profiles, dict) or len(cargo_profiles) < 2
                    or not is_optimized_profile(profile)
                    or not isinstance(codegen, dict) or set(codegen) != set(cargo_profiles)
                    or not isinstance(codegen.get("compiler"), dict)
                    or codegen["compiler"].get("opt_level") != profile["opt_level"]):
                raise CompilerProfileError(f"{binary}: missing effective Cargo library/executable profile evidence")
            for name, artifact in codegen.items():
                if (not isinstance(artifact, dict) or not isinstance(artifact.get("fingerprint"), dict)
                        or not artifact["fingerprint"].get("sha256")
                        or effective_optimization(cargo_profiles[name], artifact.get("rustflags")) != artifact.get("opt_level")):
                    raise CompilerProfileError(f"{binary}: Cargo effective profile evidence does not agree")
                fingerprint = artifact["fingerprint"]
                contents = fingerprint.get("contents")
                if (not isinstance(contents, str)
                        or hashlib.sha256(contents.encode("utf-8")).hexdigest() != fingerprint["sha256"]
                        or json.loads(contents).get("rustflags") != artifact["rustflags"]):
                    raise CompilerProfileError(f"{binary}: captured compiler fingerprint evidence changed")
            bound_receipt = dict(path=str(receipt_path), sha256=hashlib.sha256(receipt_bytes).hexdigest(),
                                 schema=CARGO_RECEIPT_SCHEMA)
        if not is_optimized_profile(profile):
            raise CompilerProfileError(f"{binary}: unoptimized or unknown compiler build profile")
        if hashlib.sha256(binary.read_bytes()).hexdigest() != identity:
            raise CompilerProfileError(f"{binary}: compiler changed during build-profile inspection")
        result = dict(binary=str(binary), sha256=identity, profile=profile)
        if bound_receipt:
            result["build_receipt"] = bound_receipt
        return result
    except (OSError, ValueError, KeyError, TypeError, subprocess.SubprocessError) as error:
        raise CompilerProfileError(
            f"optimized compiler required before validation: {error}. "
            "Rebuild with the current tools; setting Cargo variables around an existing binary does not optimize it."
        ) from error


def record_cargo_build(binary, output):
    """Bind a historical runner's actual Cargo profiles to the built executable.

    The automatic builder selects optimized profiles. Inspect both the compiler
    library and executable artifact, including cached ('fresh') Cargo records.
    Host build-script dependencies are deliberately not runtime compiler inputs.
    """
    binary = Path(binary).absolute()
    profiles, codegen = {}, {}
    for line in output.splitlines():
        try:
            item = json.loads(line)
        except ValueError:
            continue
        if not isinstance(item, dict) or item.get("reason") != "compiler-artifact":
            continue
        name = item.get("target", {}).get("name")
        executable = item.get("executable")
        matches_binary = isinstance(executable, str) and Path(executable).absolute() == binary
        if name == "compiler" or matches_binary:
            if name != "compiler" and not any(
                isinstance(filename, str) and Path(filename).absolute() == binary
                for filename in item.get("filenames", [])
            ):
                raise CompilerProfileError("Cargo executable is not bound to its artifact filenames")
            level = item.get("profile", {}).get("opt_level")
            profiles[name] = level
            codegen[name] = artifact_codegen(item)
    if "compiler" not in profiles or len(profiles) < 2:
        raise CompilerProfileError("Cargo did not identify both the compiler library and executable profiles")
    receipt = dict(schema=CARGO_RECEIPT_SCHEMA, binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),
                   profile=dict(schema=SCHEMA, opt_level=codegen["compiler"]["opt_level"]),
                   cargo_profiles=profiles, effective_codegen=codegen)
    Path(str(binary) + ".compiler-profile.json").write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")
    return receipt
