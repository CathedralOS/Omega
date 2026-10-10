#!/usr/bin/env python3
"""Materialize the canonical kernel as a local source module, never fork it."""
from pathlib import Path
import shutil
base = Path(__file__).resolve().parent
repository = base.parents[4]
source = repository / "source/library/numerics/polynomial/polynomial_horner_derivative.omg"
for project in ("scalar_cubic", "batch_quadratic", "invalid_empty"):
    shutil.copyfile(source, base / project / "polynomial_horner_derivative.omg")
