#!/usr/bin/env python3
"""Observe live Build consent and filesystem isolation on the actual CI host."""

import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools"))
import build_slot
import compiler_profile
from corpus_gate import host_target

MARKERS = (
    "restricted_consent_stopped_before_guest_execution",
    "restricted_consent_resumed_once_and_read_the_granted_bytes",
)
TEST_ROOT = "build_execution::filesystem_scope::runtime::"
COMMON_TESTS = (
    "tests::captured_read_and_output_receipt_preserve_exact_custody",
    "tests::denied_source_write_is_recorded_and_effects_cannot_change_scope",
    "live_source_tests::admitted_live_source_reads_changes_and_preserves_observation_mode",
    "live_source_tests::independent_live_root_admissions_cannot_exchange_effects",
)
LINK_TEST = "live_source_tests::admitted_live_source_cannot_be_retargeted_by_root_or_descendant_links"
# Match the production CLI's compiler stack budget. RUST_MIN_STACK controls
# spawned Rust threads, not a Windows executable's process entry stack.
COMPILER_STACK_BYTES = 256 * 1024 * 1024


def validate_sdk_output(output):
    if any(output.splitlines().count(marker) != 1 for marker in MARKERS):
        raise ValueError("public SDK did not complete both exact consent controls")


def validate_test_events(output, target):
    events = [json.loads(line) for line in output.splitlines() if line.strip()]
    observed = [row["name"] for row in events
                if row.get("type") == "test" and row.get("event") == "ok"]
    expected = [TEST_ROOT + name for name in COMMON_TESTS]
    if not target.startswith("windows_"):
        expected.append(TEST_ROOT + LINK_TEST)
    suites = [row for row in events if row.get("type") == "suite" and row.get("event") == "ok"]
    if sorted(observed) != sorted(expected) or len(suites) != 1 or suites[0].get("failed") != 0:
        raise ValueError("filesystem tests failed or did not execute the complete host roster")
    return observed


def git(*arguments):
    return subprocess.check_output(["git", *arguments], cwd=ROOT, text=True).strip()


def build_command(tool, name, arguments, *, windows):
    if windows and name == "build_charge":
        if arguments[0] != "build":
            raise ValueError("SDK stack provisioning requires its ordinary build selection")
        return [tool, "rustc", *arguments[1:], "--message-format=json", "--", "-C",
                "link-arg=/STACK:" + str(COMPILER_STACK_BYTES)]
    return [tool, *arguments, "--message-format=json"]


def build(output, name, arguments):
    tool = "mbx" if shutil.which("mbx") else "cargo"
    command = build_command(tool, name, arguments, windows=os.name == "nt")
    (output / (name + "-build-command.json")).write_text(json.dumps(command) + "\n",
                                                        encoding="utf-8")
    with build_slot.hold(ROOT):
        result = subprocess.run(command, cwd=ROOT,
                                capture_output=True, text=True,
                                env=compiler_profile.optimized_environment(), timeout=5400)
    (output / (name + "-build.jsonl")).write_text(result.stdout, encoding="utf-8")
    (output / (name + "-build.stderr")).write_text(result.stderr, encoding="utf-8")
    result.check_returncode()
    records = [json.loads(line) for line in result.stdout.splitlines() if line.startswith("{")]
    artifacts = [row for row in records if row.get("reason") == "compiler-artifact"
                 and row.get("executable")
                 and row.get("target", {}).get("name", "").replace("-", "_") == name]
    if len(artifacts) != 1 or not records[-1].get("success"):
        raise ValueError("Cargo did not identify exactly one completed " + name + " executable")
    artifact = artifacts[0]
    if str(artifact["profile"]["opt_level"]) not in compiler_profile.OPTIMIZED:
        raise ValueError("Cargo reported an unoptimized control executable")
    source = Path(artifact["executable"])
    binary = output / (name + source.suffix)
    shutil.copy2(source, binary)
    return binary, artifact["profile"]


def execute(binary, arguments, output, name, *, timeout=900):
    result = subprocess.run([str(binary), *arguments], cwd=ROOT,
                            capture_output=True, text=True, timeout=timeout)
    (output / (name + ".stdout")).write_text(result.stdout, encoding="utf-8")
    (output / (name + ".stderr")).write_text(result.stderr, encoding="utf-8")
    result.check_returncode()
    return result.stdout


def main():
    root = ROOT / "build/compiler-pilot"
    root.mkdir(parents=True, exist_ok=True)
    output = Path(tempfile.mkdtemp(prefix="build-consent-", dir=root))
    report = dict(schema="omega-build-consent-controls-v1", passed=False)
    try:
        revision = git("rev-parse", "HEAD")
        if git("status", "--porcelain") or os.environ.get("GITHUB_SHA", revision) != revision:
            raise ValueError("Build controls require the clean exact candidate")
        target = host_target()
        report.update(candidate=revision, tree=git("rev-parse", "HEAD^{tree}"), target=target,
                      toolchain=subprocess.check_output(["rustc", "-Vv"], text=True).strip(),
                      sdk_process_stack_bytes=COMPILER_STACK_BYTES if os.name == "nt" else None)
        sdk, sdk_profile = build(output, "build_charge",
                                 ["build", "--locked", "-p", "compiler", "--example", "build_charge"])
        reported_profile = compiler_profile.require_optimized(sdk)
        units, unit_profile = build(output, "package_compilation",
                                   ["test", "--locked", "-p", "package-compilation", "--lib", "--no-run"])
        identities = {str(path): hashlib.sha256(path.read_bytes()).hexdigest() for path in (sdk, units)}
        unit_output = execute(units, ["build_execution::filesystem_scope::", "-Z", "unstable-options",
                                     "--format=json", "--test-threads=2"], output, "filesystem")
        tests = validate_test_events(unit_output, target)
        sdk_output = execute(sdk, ["--restricted-consent"], output, "consent")
        validate_sdk_output(sdk_output)
        if git("rev-parse", "HEAD") != revision or git("status", "--porcelain"):
            raise ValueError("source changed while executing Build controls")
        if any(hashlib.sha256(Path(path).read_bytes()).hexdigest() != digest
               for path, digest in identities.items()):
            raise ValueError("a retained control executable changed during execution")
        report.update(passed=True, tests=tests, sdk_controls=list(MARKERS), executables=identities,
                      sdk_cargo_profile=sdk_profile, sdk_reported_profile=reported_profile,
                      unit_cargo_profile=unit_profile)
        return 0
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        report["error"] = str(error)
        print("build-consent-controls: " + str(error), file=sys.stderr)
        return 1
    finally:
        text = json.dumps(report, indent=2) + "\n"
        (output / "build-consent-controls.json").write_text(text, encoding="utf-8")
        (root / "build-consent-controls.json").write_text(text, encoding="utf-8")


if __name__ == "__main__":
    sys.exit(main())
