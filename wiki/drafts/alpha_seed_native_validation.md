# Alpha seed container native validation

Status: host-limit ledger for board item ALPHA-SEED-CONTAINER-NATIVE-VALIDATION
(TASKS.md, platform/cross-host section — structurally gated, document host
limits). This note records which hosts have executed each audited Alpha seed
container, what evidence stands behind each cell, and what would discharge the
open legs. It authorizes no new gate, seed, or host support.

Affected subjects: [Alpha tape executor](../../bootstrap/0_alpha/README.md) and
its [execution semantics](../../bootstrap/0_alpha/SEMANTICS.md),
[Alpha tests](../../tests/alpha/README.md), and the
[Alpha/Beta edge gate](../../tests/bootstrap/alpha-beta-edge.sh).

## Subjects

Two audited native seed containers, selected by
[`tools/bootstrap/alpha/seed_env.sh`](../../tools/bootstrap/alpha/seed_env.sh)
on `uname -s`-`uname -m` and bound by exact size + SHA-256 before every stamp:

| Container | Host | Bytes | SHA-256 |
| --- | --- | ---: | --- |
| `bootstrap/0_alpha/alpha_arm64_macos` | macOS arm64 | 16,942,368 | `3a9cc3112f9f7645fca00716c347865d1b160f56d458fb6340c66f237d1ae616` |
| `bootstrap/0_alpha/alpha_x64_windows.exe` | Windows x86-64 | 16,782,336 | `4ee9ee0f97c1b11c5a7ef32ffd05f1eeb193d1e9b327cb89df9ac54431aad701` |

`Darwin-arm64` selects the Mach-O seed (hole at file offset 32,768, mandatory
`codesign -f -s -` after stamping); every other `uname` selects the Windows
seed (hole at 5,120, no signing). Hosts that can run neither binary get a
container they cannot execute — the gates below refuse explicitly rather than
mis-reporting (85bb386ffe3).

## Gates that establish native validation

- `sh tests/alpha/conformance.sh` — stamps copies of the selected seed and
  executes 34 hand-built bytecode cases pinning all 21 opcodes and their edges
  (exit code + exact stdout, empty stderr, SIGILL-132 for Trap), the shared
  `io-registers.hex` host-scratch/register-isolation fixture (`AB` → exit 0,
  stdout `ABCDEF`), and `bounds.py` native mode including `--stack-only`
  full-profile loops (33,554,432 calls / 201,326,592 returns, not a reduced
  profile). Refuses with exit 2 on any host that is not `Darwin-arm64` or
  `MINGW*-x86_64`/`MSYS*-x86_64`.
- `sh tests/bootstrap/alpha-beta-edge.sh [--edge]` — the per-platform
  acceptance gate: seed behavior end to end plus exact Beta compiler
  construction; without `--edge`, also the native-source provenance rebuild
  (`alpha_arm64_macos.s` via CommandLineTools clang on macOS, or the committed
  `forge.py --check` reconstruction of `alpha_x64_windows.exe`, which runs on
  any Python-3 host). The seed-execution legs report UNAVAILABLE rather than
  failed on unsupported hosts.
- Host-portable but not native validation: the independent Python reference
  (`tests/alpha/reference/diamond-py.sh`, `bounds.py --reference` — 72/72 on
  Linux x86-64) and the forge provenance check. These cover semantics and
  byte-reconstruction evidence; they never execute the audited containers.

## Validation matrix (as of 27deadf4122)

| Host | `alpha_arm64_macos` | `alpha_x64_windows.exe` |
| --- | --- | --- |
| macOS arm64 | Executed: conformance, io-registers, bounds (incl. `--stack-only`), and edge-gate behavior legs pass; source rebuild from `alpha_arm64_macos.s` compares byte-exact modulo signature. | Not applicable (not selected). |
| Windows x64 (Git Bash + Python 3) | Not applicable. | Outstanding: no recorded native run. Audited listing reconstruction (`forge.py --check`) is provenance evidence, not runtime validation; the README states this explicitly. |
| Linux x86-64 and other hosts | No audited seed; gates exit 2. Python reference + forge check only. | Selected by `seed_env.sh` but not executable; gates exit 2. Python reference + forge check only. |

## Open legs and what discharges them

- **Windows x64 native execution** — tracked by ALPHA-WINDOWS-CONFORMANCE-HOST:
  `tests/bootstrap/alpha-beta-edge.sh` and `tests/alpha/reference/diamond-py.sh`
  on a Windows x64 host; `io-registers.hex` must exit 0 with stdout `ABCDEF`
  for stdin `AB`, retaining bounds/Trap observations and register preservation
  through host I/O. Until that runs, the d5a5d1f754f scratch/register-alias
  relocation is verified only on macOS arm64 and the reference.
- **V5 startup-allocation path** — 71ab6e24ada moved `M` (128 GiB semantic
  memory) to startup allocation (`kernel32!VirtualAlloc` on Windows, `mmap` on
  macOS arm64) and re-bound both seed identities. The macOS leg was validated
  on that change's host; the Windows `VirtualAlloc` path inherits the same
  outstanding Windows runtime leg.
- **New host support** — a Linux (or other) native leg requires an audited seed
  for that host plus a `uname` branch in `seed_env.sh`; the gates already
  refuse cleanly. Nothing in this item authorizes one.

## Observation limits

The exit-2 refusals mean an unsupported host is now distinguishable from a
broken VM. A passing reference run or forge check is not evidence the audited
container executes — only the native gates on the seed's own host are. Per the
conformance README, these tests also do not discharge native correspondence
proofs; they pin executable behavior against the audited identities.
