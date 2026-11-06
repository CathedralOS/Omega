# Filesystem release contract: current evidence and remaining proof

The unchanged Windows canonicalization customer now returns its authored exit
70 in both native execution and Terminal execution. That demonstrates the
customer's file, directory, missing-path and cleanup behavior; it does not prove
that arbitrary handle use obeys the complete release contract.

This is scoped evidence for **CANONICALIZE-SCALAR-RANGE-CONTRACTS** and
**FILESYSTEM-PLATFORM-SERVICES**, not closure of either parent acceptance.
The governing obligations remain the
[bounded occurrence-specific release proof](../../spec/build/permissions.md#bounded-occurrence-specific-release-proof).

## Observed Windows customer

The customer is
[`windows_canonicalize_exit`](../../../tests/omega/pass/filesystem/windows_canonicalize_exit/main.omg).
Its source, Build and exit-70 oracle were frozen at
`c2c57dbbc911f2384ba3233f33f456ae6c5c610a` and remained unchanged in both runs:

| Input | SHA-256 |
| --- | --- |
| `main.omg` | `05a70c6f73cc568af685a883324c06f4d8b1cde585736673b5cfb005397e0b0e` |
| `build.omg` | `19e14110cb8c33d6fa3ddfd566e9aeb7512fc152cfd358ad2e0df04737a9b960` |
| `expected_native_exit.txt` | `6442bc26a7c562f5afe6467dab36365c709909f6a81afcecfc0c25cff0f1bab0` |

[Baseline run 42288076952](https://github.com/CathedralOS/Omega/actions/runs/42288076952)
used an optimized Windows producer from that source, executable SHA-256
`26589cdde55a30f4aad366b780f710a6a61ee60f204bb188a2edbbe2b3d644ad`.
Its ordinary CLI `update` exceeded the 600-second limit after 600.453 seconds.
Neither execution engine ran. The only retained stderr observation said the
standard-library review had been encoded; completed stage times were buffered
until the outer review returned and were lost on termination.

[Attributed run 42295325098](https://github.com/CathedralOS/Omega/actions/runs/42295325098)
used source `d6693f0c32f65e83a0d91a6ef4d406583d524a9a`, which changes only
opt-in review timing output. Its separately authenticated producer SHA-256 was
`d8ab75162016f09c1531c232ef072ecb82ba397f23b168387a51f1b071ccf53b`.
Both producers reported optimization level 1 and the pinned Windows toolchain
`rustc 1.100.0-nightly (a69a63265 2026-09-03)`.

The exact focused dispatch was:

```sh
gh workflow run compiler-pilot.yml --ref work/scalar-windows-release-20261106 \
  -f native_host=windows_x86_64 \
  -f cli_fixtures=pass/filesystem/windows_canonicalize_exit \
  -f checking_only=false -f phase_profile=true
```

The ordinary CLI consumer copied unchanged inputs into fresh scratch storage,
used the existing review protocol and retained these actual outcomes:

| Stage | Observed result | Wall time | Existing limit |
| --- | --- | --- | --- |
| `update --target windows_x86_64` | Exit 3, awaiting review | 527.265 s | 600 s |
| Accept pending review rows, then `update --resume` | Exit 0 | 9.469 s | 600 s |
| `run --both --keep main.omg` | Wrapper 70, native 70, Terminal 70 | 156.203 s | 240 s |

The 180-file input-manifest digest was identical in both runs:
`2b5c864ed1c03cbdae96c96753cb7f40d4ffebc71c4b6505271449adee5bb544`.
The successful run's initial discovery review spent 383.878 seconds on the
standard library and 126.493 seconds on the customer. Its subsequent bound
review spent 3.963 and 2.041 seconds respectively. Resume reused all four
completed review occurrences and spent 0.346 seconds in candidate review.
These figures locate the measured work; they do not establish why the earlier
run exceeded its limit or show a compiler speedup from diagnostic output.

Small authenticated records and bounded raw stderr members were retained under
`build/scalar-continue-20261102/windows-canonicalize-{baseline-42288076952,attributed-42295325098}/`.
The downloader's local receipt initially used a Linux template label; the
receipt records its correction to Windows explicitly. Downloaded official JSON
and stderr members remain unchanged. Source identities above describe actual
observations, including when later changes affect comments only.

## Current implementation and remaining obligations

At the measured sources,
[`windows_filesystem_path_query.omg`](../../../source/library/std/targets/windows_x86_64/windows_filesystem_path_query.omg)
uses `CreateFileW` through the exact metadata-open requirement. It requests
read-attributes access, sharing mode 7 (read/write/delete), `OPEN_EXISTING`,
and backup semantics with anonymous SQOS, excluding `DELETE_ON_CLOSE`.
Only a nonzero, non-invalid acquired handle reaches the query and following
close. The old customer header describing `CreateFileA` is historical.

That source sequence is not itself retained proof of its complete lifecycle.
The measured compiler's generic filesystem policy refuses to mint an empty
mechanism classification for an unconstrained ordinary-release cohort.
The earlier draft's named release-contract constructors and claimed customer
stops are not present evidence at these sources.

The remaining derivation must establish and retain the exact acquired handle
occurrence and arguments, identity through aliases and joins, preservation by
every intervening handle-taking call, one applicable ordinary release, no later
use, and explicit external-environment assumptions. It must bind the evidence
to the exact mechanism and checked contract. Empty reach, query-only reach,
method names and an integer handle domain do not establish those obligations.
Preserve read/write/delete sharing; deletion attached to the released handle
must prevent ordinary-empty classification.

The customer's missing-path leg and separate `FailedOpen::main` exercise
NotFound. They do not prove exactly one close or no later use. Existing
filesystem authority rejects check missing service reach; they are not
release-lifecycle falsifiers. Terminal's private guest-handle tracking rejects
unknown or uncertain handles at runtime, which likewise does not establish the
static proof. Failed acquisition, escaping or substituted handles, unproved
intervening calls, duplicate release, later use and stale proof reuse still
need fact-sensitive rejection evidence before this release contract closes.
