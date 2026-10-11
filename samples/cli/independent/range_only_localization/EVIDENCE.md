# ADD09 evidence and reproduction

The application is private to this directory. It imports only published std
Console facilities and core numeric conversion; it has no worker or numerical
library dependency. Source comparison and ownership are in `ASSIGNMENT.md`.
The model and limitations are in `README.md`.

The original oracle was frozen before any Omega execution: 18 cases, manifest
SHA256 `00b3e7636698202552def906938d3510be5c8656fca67628e6f2e4d540075a65`.
An independent anchor-first annulus set intersection confirms the constructed
lattice positions. The first two exact circles intersect at (3,4) and (3,-4);
the third selects (3,4). Integer tolerance is zero; the independent continuous
witness tolerance is 1e-12. A separate frozen wide-band case has 121 candidates;
three separately frozen cases cover LF and EOF at the 128-byte read capacity.
Neither validator regenerates expected output.

## Witnessed development controls

Native execution initially declined the guarded fixed-array-to-slice argument
on main `9a0463cc42af5f7178372c169ae55e528fe43878`; the same complete application
reproduced it on then-current published main
`2f145503f24ecc1cac43588dc1c593dc069f9e7d`. The owning Psi structural argument
producer needed to form a view descriptor at the argument occurrence, and its
consumer needed to keep that exact retained root ahead of borrow peeling.
The general repair has two corpus controls:

- `pass/slices/fixed_array_slice_state_arguments_exit`: a local array and a
  mutable array field become state slices, preserve backing writes, print
  `3456\n`, and exit 70 through native and Terminal.
- `fail/borrows/fixed_array_slice_state_immutable_rejected`: an immutable
  referent cannot lend writable access. The exact existing diagnostic is pinned.

The unchanged `pass/slices/local_array_element_view_exit` also passed native
and Terminal after the producer/consumer repair. Only the new controls' corpus
rows were added; existing ledger rows and main's later entries were preserved.
A scoped `typed-trees-to-checked-trees` Clippy run exited 0 in 1m07s; it reported
234 existing warnings, including an unchanged `?` suggestion at
`structural_arguments.rs:839`. No warnings were suppressed or unrelated lint
changes applied.

A recursive private decimal formatter reached the existing unsupported recursive
Unit closure boundary. The next scalar iterative formatter exposed
`RegisterAllocation(RuntimeSpill(Homes(NoCommonCandidate { function: 9,
register: 5 })))` on the same main base. An optional crossing-spill experiment
still failed and was withdrawn completely. No allocator or backend change is
retained. The delivered private formatter prepares decimal bytes in one
`Decimal` record before writing them with the existing Console provider; it
retains the same signed decimal output and handles the nonpositive magnitude
of i32::MIN without overflow. Its guarded transitions use ordinary checked
Omega arithmetic and array bounds.

The complete source with this formatter passed native and Terminal for the
unique case. The real admitted sample, compiled with compiler SHA256
`0c78368c0f77d755778f01c95c2e13aa3429c34deeef0346dc9e7455d8f00609`
from the repaired 2f base, produced `build/omega-program`, SHA256
`6a1ebc52082ed32bb3e5d9c36949755ac5525afe6f1ea8d567bb8b84469a8723`.
All 28 native runtime cases passed exact stdout, empty stderr and exit checks.
This is pre-integration evidence; final integrated identities and per-case
observations are preserved in the review bundle's validation manifest and logs.

## Integrated validation commands

The isolated branch was rebased onto published main
`a5d20d6a7faeaf56acabe903a4e78275760d0cee`. Build metadata binds the existing
Linux x86_64, Windows x86_64 and macOS arm64 entry providers; execution evidence
is for Linux x86_64 only. The executor is UID 1000, Linux x86_64, five CPUs,
18440136 KiB memory. One Rust build and one Omega execution are used at a time.
The pinned toolchain is nightly-2026-09-04, Rust build jobs 4, corpus jobs 1.

```sh
source /workspace/cloud-setup/activate.sh
export CARGO_TARGET_DIR=/workspace/add09-opt-target
export CARGO_PROFILE_DEV_OPT_LEVEL=1
export OMEGA_CORPUS_JOBS=1
export OMEGA_LIBRARY_ROOT="$PWD/source/library"
mbx build -p omega
omega update --project samples/cli/independent/range_only_localization --target linux_x86_64
# If a pending review is produced, inspect its exact local decisions and resume
# through the normal package workflow; no lock or source guard is hand-edited.
omega --check --target linux_x86_64 samples/cli/independent/range_only_localization/main.omg
omega --target linux_x86_64 samples/cli/independent/range_only_localization/main.omg
python3 -B samples/cli/independent/range_only_localization/validate.py \
  samples/cli/independent/range_only_localization/build/omega-program
python3 tools/corpus_gate.py --both \
  --filter pass/slices/fixed_array_slice_state_arguments_exit,pass/slices/local_array_element_view_exit \
  --golden /tmp/add09-integrated-slices.txt --record
python3 -B samples/cli/independent/range_only_localization/validate_terminal.py \
  /absolute/path/to/the/verified/corpus_runner
omega --check --target linux_x86_64 \
  samples/cli/independent/range_only_localization/intended_invalid/rejected.omg
```

The separate final command must exit 1 with the authored unproved array-index
fragment from `intended_invalid/expected_error.txt`. The standalone control has
its own complete entrypoint and build metadata; it is never part of the valid
application entrypoint. The validators save actual streams or corpus stream
comparison records and exact commands under the ignored `build/` directory.
The transfer bundle includes these artifacts rather than relying on local paths.

Default sandbox execution exposed synthetic root ownership rejected by cache
custody. Ordinary per-command escalated compiler execution was individually
approved and retained UID 1000 with real UID 0 ownership of `/`. No ownership,
cache-custody, proof, source-admission or provider guard was altered. Executor
disconnections were followed by same-executor readiness checks; no replacement
worker or executor was created. The normal main-only landing queue follows
independent parent/coordinator review, as explicitly requested.
