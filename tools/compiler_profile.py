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
import subprocess


SCHEMA = "omega-compiler-build-profile-v1"
OPTIMIZED = {"1", "2", "3", "s", "z"}


class CompilerProfileError(ValueError):
    pass


def optimized_environment():
    """Use the same optimized profiles for automatic CLI and corpus builds."""
    return dict(os.environ, CARGO_PROFILE_DEV_OPT_LEVEL="1", CARGO_PROFILE_TEST_OPT_LEVEL="1")


def is_optimized_profile(profile):
    return (isinstance(profile, dict) and profile.get("schema") == SCHEMA
            and isinstance(profile.get("opt_level"), str) and profile["opt_level"] in OPTIMIZED)


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
            profile = receipt["profile"]
            cargo_profiles = receipt.get("cargo_profiles")
            if (not isinstance(cargo_profiles, dict) or len(cargo_profiles) < 2
                    or not is_optimized_profile(profile)
                    or cargo_profiles.get("compiler") != profile["opt_level"]
                    or not all(isinstance(level, str) and level in OPTIMIZED for level in cargo_profiles.values())):
                raise CompilerProfileError(f"{binary}: missing optimized Cargo library/executable profile evidence")
            bound_receipt = dict(path=str(receipt_path), sha256=hashlib.sha256(receipt_bytes).hexdigest())
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
    profiles = {}
    for line in output.splitlines():
        try:
            item = json.loads(line)
        except ValueError:
            continue
        if not isinstance(item, dict) or item.get("reason") != "compiler-artifact":
            continue
        name = item.get("target", {}).get("name")
        if name == "compiler" or item.get("executable") == str(binary):
            level = item.get("profile", {}).get("opt_level")
            if not isinstance(level, str) or level not in OPTIMIZED:
                raise CompilerProfileError(f"{name}: Cargo built an unoptimized or unknown compiler profile ({level})")
            profiles[name] = level
    if "compiler" not in profiles or len(profiles) < 2:
        raise CompilerProfileError("Cargo did not identify both the compiler library and executable profiles")
    receipt = dict(binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),
                   profile=dict(schema=SCHEMA, opt_level=profiles["compiler"]), cargo_profiles=profiles)
    Path(str(binary) + ".compiler-profile.json").write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")
    return receipt
