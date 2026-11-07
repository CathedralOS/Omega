"""Find the native outcome a fixture authors for the selected target.

A target-specific file overrides the portable file. Exit promises and expected
publication refusals remain separate: a refusal never proves execution.
"""

from pathlib import Path


def _target_file(fixture: Path, name: str, target: str) -> Path:
    specific = fixture / f"{name}.{target}.txt"
    return specific if specific.is_file() else fixture / f"{name}.txt"


def native_exit_oracle(fixture: Path, target: str) -> Path:
    return _target_file(fixture, "expected_native_exit", target)


def native_rejection_oracle(fixture: Path, target: str) -> Path:
    return _target_file(fixture, "expected_native_rejection", target)
