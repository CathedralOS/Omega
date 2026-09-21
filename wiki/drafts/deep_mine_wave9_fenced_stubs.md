# Deep-mine wave 9: fenced stub ledger

Ledger for `NEW-DMS9-FENCED-STUB-LEDGER`. Recorded against `6f91898606` on
linux x86-64, claims-registry snapshot taken ~2026-09-21T08:24Z (137 live
claims at snapshot).

## What this ledger covers

The wave-9 deep mine appended several hundred bare "mined candidate" stubs to
`TASKS.md`. The sweep commits `d6a0625f6b` (145 contentless stubs),
`50559da3ab` (116 verified-passed rows), `091f5ba75c`/`41a7bde098` (sweeps
A/B, 55+59) and `a51dd959f7` (orphan sweep) retired most of them. This ledger
records the residual class: stubs whose board content was consumed, retired,
or verified but whose **implementing surfaces remain fenced** — a `claims.py
claim` on the surface returns exit 2 with a live sibling claim — so no slice
is landable even where real work remains. A stub that is merely retired or
covered is not in this ledger; a stub blocked by a host gate rather than a
claim fence is listed separately.

## Fenced stubs (measured at snapshot)

| Stub | Row status | Blocking fence (item / owner / expiry) |
| --- | --- | --- |
| `PIPELINE-REWRITE-CATALOG-WIRING` | no row — claim-lane name cited inside sibling annotations (TASKS.md ~:9793, ~:9930); the SELECTED-REWRITE-CATALOG wiring leg (~38 orphan entrances → `Optimization` vocabulary + `module_catalog` entries) | `rewrites/module_catalog.rs`, `selected_optimization.rs`, `selected_optimization/*` under REWRITE-CATALOG-ADMISSION (Devin / z134, exp 14:31Z); item name held by PIPELINE-REWRITE-CATALOG-WIRING (zergling-182, exp 14:48Z); earlier fenced by SELECTED-REWRITE-CATALOG-DISPOSITION (exp 13:52Z) |
| `PIPELINE-SPILL-FAMILY-ORPHANS` | no row — stub citation on PIPELINE-OWNER-CONSOLIDATION covering the `unsequenced_spill_stages` family | `selected-instructions-to-register-homes/src/unsequenced_spill_stages` wholesale-fenced by POC-SPILL-FAMILY-SEQUENCING (Jarod / swarm-w9-poc-spill-family-sequencing, exp 06:53Z) |
| `UNSEQUENCED-SPILL-FAMILY-DISPOSITION` | retired by `d6a0625f6b`; content folds into UNSEQUENCED-SPILL-STAGES-DISPOSITION ("no independent slice") | same `unsequenced_spill_stages` fence as above |
| `TWO-AXIS-TERMINAL-AUTHORITY-REVIEW` | live row (TASKS.md ~:4879), partially landed; remaining legs are receiver rows, Filesystem-summary retirement, release-cohort close, cli witness | `packages/manager/tests` under BUILD-PACKAGES-GATE fixtures (Zergling-129, exp 07:44Z); `native-realization/terminal_authority_policy` under FILESYSTEM-RELEASE-CONTRACT (zergling-z27, exp 14:14Z); cli customer leg additionally under X86-FMA-PROVIDER-TRANSPORT + CALLBACK-PRIVATE-MATERIALIZATION fences and a macOS host gate |
| `SQUALR-HEADLESS` | live row (TASKS.md ~:99); next delivery is fast-forward publish of pin `5b0307c352` onto the application's `main` + `verify.py native` per matching host | `samples/apps/squalr` wholesale-fenced by SQUALR-PLUGIN-IMPLEMENTATIONS (Devin / z28-squalr-plugins, exp 16:17Z) at claim time — the fence covers all port sources plus `tools/verify.py` |
| `TRANSFORM-CODEC-RELOCATION` | consumed stub (TASKS.md ~:13974) — work continues on DURABLE-CODEC-RELOCATION | two of three codec sites landed (`32e4ff98f4`, `2e3c662c32`); remaining `native-realization/optimized_semantic_wrapper_object/codec.rs` fenced by PIPELINE-WRAPPER-OBJECT-ORPHAN (~22:46Z at row-verification time) and deferred on PIPELINE-OWNER-CONSOLIDATION |
| `EXACT-MACHINE-SIMPLIFICATIONS` per-module catalog legs | live row sub-legs (TASKS.md ~:13386): literal_*, constant_*, boundary_*, dead_compare, interchange/relocation families, load_forwarding, store_motion, local_schedule, dead_store, runtime_rematerialization, runtime_spill | whole `selected-instructions-to-selected-instructions/` crate fenced under POC-REWRITE-ORPHANS (Zergling-91, exp 22:33Z); `rewrites/{mod.rs,module_catalog.rs}` under SELECTED-STAGE-RULE-CATALOG (Devin/z88, exp 02:32Z) at row-verification time — currently `module_catalog.rs` under REWRITE-CATALOG-ADMISSION |

## Fenced claim-lane names (no board row exists)

These names appear only as claim-lane owner citations inside sibling
annotations, yet their surfaces stayed live-fenced — every `claim` attempt
returns exit 2 and the correct verdict is `blocked`, not `already_resolved`:

- `PIPELINE-REWRITE-CATALOG-WIRING` (in ledger above)
- `PIPELINE-SPILL-FAMILY-ORPHANS`, `UNSEQUENCED-SPILL-FAMILY-DISPOSITION`
  (in ledger above)
- `PIPELINE-CRATE-SWEEP` — resolved-then-retired; the sweep exists as the
  landed self-auditing architecture gate (`stage_crate_ownership.rs`,
  `route_conformance.rs`, `pipeline_rewrites_ownership_audit.md`), so it is
  out of the fenced class — listed here only because the name still circulates
  as a stale claim-lane citation in the RC-gate annotation.

## Host-gated (not claim-fenced — distinct class)

- `RC-NATIVE-MATRIX-WINDOWS-X64` — residual leg executes the emitted PE on a
  Windows x86-64 host; compile-side lanes verified green. No linux_x86_64
  slice exists; sibling `RC-WINDOWS-X64-NATIVE-ROW` records the same gate.
- Benchmark per-host rows (`BENCHMARK-CROSS-HOST-ROWS` runtime legs for
  macos_arm64 / windows_x86_64 / linux_arm64 / uefi_x86_64) — foreign-runner
  bound, not fence-blocked.

## Registry mechanics observed this wave

- Fences rotate on short cycles: a surface blocked at one read may be open a
  few minutes later, and a surface open in `claims.py status` may be fenced by
  the time `claim` runs (SQUALR-PLUGIN-IMPLEMENTATIONS conflicted at claim
  time yet was absent from a `status` read taken seconds apart — reads and
  writes do not see identical ref states under load).
- The `claim` write path (`ls-remote` + push on
  `refs/coordination/omega-claims/main` via the coordination remote) has
  intermittently hung under swarm load this wave (two consecutive 120s
  timeouts observed); `status` reads and `git fetch` stayed responsive.
- Item-name claims with empty `paths` occupy the item lane without fencing
  any file surface; several sibling annotations cite them as stale fences.
