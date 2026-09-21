# Deep-mine wave 9: fenced stub ledger

Ledger for `NEW-DMS9-FENCED-STUB-LEDGER`. First recorded against
`6f91898606`; refreshed against `c3dd8016a7` on linux x86-64,
claims-registry snapshot taken ~2026-09-21T08:44Z (155 live claims at
snapshot; earlier snapshot ~08:24Z had 137). Re-refreshed against
`891eb5c584` on linux x86-64, claims snapshot ~2026-09-21T08:53Z (160
live claims).

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
claim fence is listed separately, and a stub whose fence drained between
snapshots is moved to *Drained since first snapshot*.

## Fenced stubs (measured at snapshot)

| Stub | Row status | Blocking fence (item / owner / expiry) |
| --- | --- | --- |
| `PIPELINE-REWRITE-CATALOG-WIRING` | no row — claim-lane name cited inside sibling annotations (TASKS.md ~:12063, ~:12333); the SELECTED-REWRITE-CATALOG wiring leg (~38 orphan entrances → `Optimization` vocabulary + `module_catalog` entries) | `rewrites/module_catalog.rs`, `selected_optimization.rs`, `selected_optimization/*` under REWRITE-CATALOG-ADMISSION (Devin / z134, exp 14:31Z); item name held by PIPELINE-REWRITE-CATALOG-WIRING (zergling-182, exp 14:48Z) |
| `TWO-AXIS-TERMINAL-AUTHORITY-REVIEW` | live row (TASKS.md ~:5223), partially landed; remaining legs are receiver rows, Filesystem-summary retirement, release-cohort close, cli witness | at refresh: `packages/manager/tests` fence (BUILD-PACKAGES-GATE) drained — only `manager/tests/repository_build_declarations.rs` now held under NEW-CPMS-DECLARED-PATH-RESOLUTION-PIN (Devin / ea735f37, exp 16:25Z); `native-realization/terminal_authority_policy` fence (FILESYSTEM-RELEASE-CONTRACT) drained — no live claim on the surface; cli customer leg remains under X86-FMA-PROVIDER-TRANSPORT (Jarod / swarm-w9, exp 09:07Z) + CALLBACK-PRIVATE-MATERIALIZATION (Zergling-55, exp 09:13Z) fences and a macOS host gate. At the 08:53Z re-refresh the item lane itself is also held pathless (zergling-39, exp 16:50Z), the two cli-leg fences were still live, and the receiver/Filesystem legs' surfaces stayed unfenced |
| `SQUALR-HEADLESS` | live row (TASKS.md ~:99); next delivery is fast-forward publish of pin `5b0307c352` onto the application's `main` + `verify.py native` per matching host | `samples/apps/squalr` still wholesale-fenced, owner rotated: the earlier SQUALR-PLUGIN-IMPLEMENTATIONS claim (z122) narrowed to `TASKS.md` only; the tree fence now sits under SQUALR-DEBUG-ASSERTIONS (Devin / z35-squalr-debug-assertions, exp 16:25Z) |
| `TRANSFORM-CODEC-RELOCATION` | consumed stub (TASKS.md ~:15028) — work continues on DURABLE-CODEC-RELOCATION | two of three codec sites landed (`32e4ff98f4`, `2e3c662c32`); the PIPELINE-WRAPPER-OBJECT-ORPHAN fence on the remaining `native-realization/optimized_semantic_wrapper_object/codec.rs` drained, and the UEFI-PHYSICAL-SEMANTIC-ENTRY fence on the whole `native-realization/src/optimized_semantic_wrapper_object` directory drained at its 08:44Z expiry (gone at the 08:53Z snapshot). The directory's keep/move decision instead sits under pathless item fences — POC-NATIVE-WRAPPER-RELOCATION (z182, exp 16:00Z), SEMANTIC-WRAPPER-OWNER-RELOCATION (f8ad694c, exp 16:39Z), OPAQUE-BY-VALUE-BOUNDARY-ABI (z115, exp 15:16Z) — while the codec surfaces stay inside the working item's own DURABLE-CODEC-RELOCATION claim (Devin / z120, exp 16:35Z) |
| `EXACT-MACHINE-SIMPLIFICATIONS` per-module catalog legs | live row sub-legs (TASKS_OPTIMIZER.md ~:718): literal_*, constant_*, boundary_*, dead_compare, interchange/relocation families, load_forwarding, store_motion, local_schedule, dead_store, runtime_rematerialization, runtime_spill | whole-crate POC-REWRITE-ORPHANS fence (Zergling-91) drained — the crate is no longer wholesale-fenced; per-surface fences remain: `rewrites/{mod.rs,module_catalog.rs}` + `selected_optimization*` under REWRITE-CATALOG-ADMISSION (z134, exp 14:31Z), `rewrites/{fixed_view,allocation_recovery,selected_lowering/literal_fold,literal_folds}` + `lib.rs` under DURABLE-CODEC-RELOCATION (z120, exp 16:35Z), `rewrites/arm_relocation*` under REWRITE-VALIDATOR-INDEPENDENCE (z50, exp 10:33Z) |

## Fenced claim-lane names (no board row exists)

These names appear only as claim-lane owner citations inside sibling
annotations. At refresh, `PIPELINE-REWRITE-CATALOG-WIRING`'s surfaces remain
live-fenced (`claim` returns exit 2; the correct verdict is `blocked`, not
`already_resolved`); the two spill-family names have moved to the drained
list below:

- `PIPELINE-REWRITE-CATALOG-WIRING` (in ledger above)
- `PIPELINE-CRATE-SWEEP` — resolved-then-retired; the sweep exists as the
  landed self-auditing architecture gate (`stage_crate_ownership.rs`,
  `route_conformance.rs`, `pipeline_rewrites_ownership_audit.md`), so it is
  out of the fenced class — listed here only because the name still circulates
  as a stale claim-lane citation in the RC-gate annotation.

## Drained since first snapshot

- `PIPELINE-SPILL-FAMILY-ORPHANS`, `UNSEQUENCED-SPILL-FAMILY-DISPOSITION` —
  the POC-SPILL-FAMILY-SEQUENCING wholesale fence on
  `selected-instructions-to-register-homes/src/unsequenced_spill_stages`
  drained; no live claim covers the surface at refresh. The stub content
  still folds into UNSEQUENCED-SPILL-STAGES-DISPOSITION ("no independent
  slice"), so the names are merely exhausted, not fenced.
- `COMPOSABLE-PAIR-DESCRIPTORS` — its own surface claim drained; the
  descriptor legs were already landed (`58126066cc` operand-shape axes,
  `dbf8fa8edc` unit-effects axes), and the surface is unclaimed at refresh.
- `packages/manager/tests` (BUILD-PACKAGES-GATE) and
  `native-realization/terminal_authority_policy`
  (FILESYSTEM-RELEASE-CONTRACT) fences drained — see
  `TWO-AXIS-TERMINAL-AUTHORITY-REVIEW` row for what still gates that item.
- `UEFI-PHYSICAL-SEMANTIC-ENTRY` — the broad native-realization fence
  (including `optimized_semantic_wrapper_object`) drained at its 08:44Z
  expiry; at the 08:53Z snapshot the item holds no claim at all. The
  wrapper-object directory's codec leg stays fenced under the wrapper
  item-lane claims and DURABLE-CODEC-RELOCATION; see the
  TRANSFORM-CODEC-RELOCATION row.

## Host-gated (not claim-fenced — distinct class)

- `RC-NATIVE-MATRIX-WINDOWS-X64` — residual leg executes the emitted PE on a
  Windows x86-64 host; compile-side lanes verified green. No linux_x86_64
  slice exists; sibling `RC-WINDOWS-X64-NATIVE-ROW` records the same gate.
- Benchmark per-host rows (`BENCHMARK-CROSS-HOST-ROWS` runtime legs for
  macos_arm64 / windows_x86_64 / linux_arm64 / uefi_x86_64) — foreign-runner
  bound, not fence-blocked.

## Registry mechanics observed this wave

- Fences rotate on short cycles: between the ~08:24Z and ~08:44Z snapshots
  three recorded fences drained outright (POC-SPILL-FAMILY-SEQUENCING,
  PIPELINE-WRAPPER-OBJECT-ORPHAN, POC-REWRITE-ORPHANS) and one narrowed to a
  board-only lane (SQUALR-PLUGIN-IMPLEMENTATIONS → `TASKS.md`), while a new
  wholesale claim appeared on `samples/apps/squalr` and a broad
  native-realization fence covered the third codec site. A surface open in
  `claims.py status` may be fenced by the time `claim` runs, and vice versa —
  reads and writes do not see identical ref states under load.
- The `claim` write path (`ls-remote` + push on
  `refs/coordination/omega-claims/main` via the coordination remote) has
  intermittently hung under swarm load this wave (two consecutive 120s
  timeouts observed at the first refresh; at the 08:53Z re-refresh the
  pattern repeated — two more 120s+ hangs and a push-race loss before
  this ledger's claim landed). `status` reads and `git fetch` stayed
  responsive throughout.
- Item-name claims with empty `paths` occupy the item lane without fencing
  any file surface; several sibling annotations cite them as stale fences.
