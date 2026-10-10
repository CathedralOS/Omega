#!/usr/bin/env python3
"""Materialize samples against the exact owned source without inventing metadata.

NF01 owns the numerics package metadata. Until its API arrives, this explicit
copy makes the independent pivot source available as an ordinary local module.
Generated projects are disposable; authored sources and failed logs are retained.
"""
from pathlib import Path
import shutil

root = Path(__file__).resolve().parent
repository = root.parents[4]
pivot = repository / 'source/library/numerics/linalg/triangular_solves.omg'
standard = repository / 'source/library/std'
for name in ('lower_multiple_rhs', 'upper_transpose_unit', 'invalid_unproved_pivot'):
    destination = root / '.prepared' / name
    destination.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(root / name / 'main.omg', destination / 'main.omg')
    shutil.copyfile(pivot, destination / 'triangular_solves.omg')
    build = (root / name / 'build.omg').read_text()
    build = build.replace('../../../../../../source/library/std', standard.as_posix())
    (destination / 'build.omg').write_text(build)
    print(destination / 'main.omg')
