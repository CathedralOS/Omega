# NF08 Welford moments and merge candidate

Owned capability: allocation-free scalar mean, unnormalized central M2,
transactional Welford updates and chunk merges, named population/sample variance.

`streaming/` consumes eight observations through a loop and checks a singleton.
`uneven_chunks/` combines 2/1/2 large-offset chunks through two merge trees and
compares a sequential stream. `invalid_unhandled_sample_result/` independently
requires type rejection when a fallible variance result is assigned to f64.
All three contain complete main.omg and build.omg sources. They depend on NF01
publishing the numerics package metadata; the proposed alias must be confirmed.

Frozen numerical fixtures and absolute tolerances: ORACLES.md.
Independent Fraction and Python formula validation: validate_math.py.
Run `python3 validate_math.py` from this directory. This establishes the math,
not Omega execution or source acceptance.

## Current qualification boundary

Base main: 9d4451128dd05885e7b9c591796b2760189851db.
Initial checkout: 6e4f0c37ecd6dbb26d228b3045d40bc6b243d0bb, clean, branch work.
Isolated branch nf08-moments; worktree /workspace/Omega/.codex/worktrees/nf08.
CPU: 5 available; load 0.00/0.00/0.00. MemAvailable 17,712,924 KiB
(~16.9 GiB), MemTotal 18,440,136 KiB. Free workspace disk ~30 GiB.
Live claim status and registration timed out; ticket
f75beb8ce8934db7911b3cc5075a333e is pending, not confirmed.

The executor maps outside UID 0 to inside UID 1000 and shows `/` as UID 65534.
The preinstalled Omega refuses cache ancestry owned by that unrelated UID.
Default home cache also fails filesystem writability. Neither failure reaches
source checking. Each route was attempted serially; native exits 200, check and
Terminal exit 1. The invalid control's intended rejection is unqualified.
Full stdout/stderr, exact argv and environment are in evidence/. CLI binary
SHA256 and unknown preinstalled build revision are recorded; no candidate
Rust compiler build or source/runtime acceptance is claimed.

The next step is an executor with valid private-cache ancestry, dependency
metadata/interfaces from NF01/NF02/NF07, then each authored route through a
compiler built at the recorded candidate/base revision. Integrate descriptor
traversal through the owners' APIs once published; do not create a second layout
or accuracy-policy implementation. See the owned api.txt for signatures.

This branch is a reviewable checkpoint; it is not ready for normal landing.
