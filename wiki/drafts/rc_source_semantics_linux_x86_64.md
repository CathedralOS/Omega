# RC source semantics — linux_x86_64 row

Witnessed row of the `RC-SOURCE-SEMANTICS` release gate on the Linux
x86-64 host. Recorded at revision `0977a4249e` (2026-09-20), host
`x86_64-unknown-linux-gnu`, cargo-nextest (mbx unavailable). The gate
command from `wiki/drafts/rust_compiler_completion.md` is
`cargo nextest run -p compiler --all-targets --no-fail-fast`: every
accepted positive fixture reaches its promised checked or product stage,
every negative fixture rejects, and the individual semantic integration
tests pass.

Verdict: **red** — `Summary [30238.885s] 3081 tests run: 1903 passed
(272 slow), 1178 failed, 0 skipped`. Every observed failure attributes
to an already owned family; no unowned residual was found.

## Failures

Failures are grouped by first reported diagnostic; per-test diagnostics
often chain several of the listed rejections, so family counts count
failed tests, not diagnostic occurrences.

| tests | family (first diagnostic, normalized) | attribution |
|-------|---------------------------------------|-------------|
| 662 | `selected ProgramEntry establishment rejoins 0 Terminal attachment identities; expected one` | Documented upstream regression: `derive_fused_program_entry_establishments` rejects `Service<R>`-fielded ProgramEntry receivers — the missing `attachment_type_identity` row is produced in t2c; stop site `omega-rust/omega/build/selected-dispatch/src/service_custody/root.rs`. Owned by **ENTRY-CONTENT-ROOTS** (intrinsically-established service carrier); recorded blocking the FMA lane (TASKS.md:5849), the Omega-parser gate (TASKS.md:5675), and CROSS-COMPILER-DIFFERENTIAL-LANE (TASKS.md:6425). |
| 80 | non-diagnostic panics/assertions | Mixed: `arena span append must be contiguous` (6), `unexpected diagnostics` respells (4), architecture review rows — `machine-emission`/`resolved-layout-to-resolved-layout` Cargo.toml dependency review (architecture_boundaries family, also counted below), `selected filtering must not change generated-source custody`, assorted single-case asserts. |
| 44 | `exact arithmetic … may overflow … operands are not provably in range` (assignment + local spellings) | **ARITHMETIC-POLICY-REALIZATION** (exact-arithmetic range proofs). |
| 28 | `native artifact identity physical pipeline failed: … Selection(Legalization(UnsupportedScalarOperation { ExactIntegerDivide / WrappingIntegerDivide … }))` | **ARITHMETIC-POLICY-REALIZATION** division lane; companion entry-binding legs owned by **DIVISION-VALUE-ENTRY-SELECTION** / **DIVISION-CANARY-ENTRY-BINDING**. |
| 27 | `native-artifact production requires one exact selected program entry` | Program-entry exactness lane — **PROGRAM-ENTRY-SELECTION-EXACTNESS** / **CANARY-EXACT-ENTRY-SELECTION** (same repair pattern as e5912f303a: fixtures carrying no `build.omg` entry binds). |
| 19+2 | `selected ProgramEntry Service field requires a selected Fused provider for boundary …` / `routed service field … has no exact Fused selected-provider-plan join` | Selected-provider-plan join; **PROVIDER-ATTACHMENT-MACHINE-PLAN** / Fused-provider lane. |
| 18 | `target physical entry requirement and schema … require either the exact bundled Windows x86-64 contract or one accepted package-owned Windows x86-64 binding, not …/std/targets/windows_x86_64/entry.omg` | `samples_compile` windows_x86_64 leg — bundled Windows entry binding not admitted; sample/windows-entry lane (**SAMPLES-COMPILE-MULTI-HOST** area). |
| 17+16+2+9 | borrow-custody rejections: `cannot make a boundary or service call while X is absent…`, `cannot transfer a non-copy value out of borrowed storage…`, `cannot store X while X is absent…` | **WRITE-ONLY-BORROW** / **BORROW-PROOF-CONVERGENCE** lanes (moved-out borrowed storage restoration). |
| 16+6(+25 secondary) | `public interface selects private data/domain` + `package X selects private machine/domain` | Package visibility enforcement — **BUILD-PACKAGES-GATE** / package-evidence lanes. |
| 13 | `indexing spelling X has unsupported selected X on a non-array, non-slice collection` | Indexing selection — **OPERATOR-MACHINE-SUPPLY** area. |
| 10 | `Lowering(Unsupported("indexed reads require a whole byte-view parameter"))` | Byte-view indexed-read lowering leg. |
| 9 | `field … names bare boundary trait … in value position; the intrinsic … carrier is the only service value spelling` | **ENTRY-CONTENT-ROOTS** bare-trait fixture spelling (documented migration to the intrinsically established service carrier). |
| 9 | `Lowering(Unsupported("composed Unit scalar call requires structural call custody"))` | Structural call custody — **WRITE-ONLY-BORROW**/`structural_call_custody.rs` (claimed under RC-REPOSITORY-BASELINE). |
| 8 | `provider selection operand does not resolve to one visible product declaration` | Provider selection visibility. |
| 8+5+3 | `cannot prove default-domain field requirement for call …` | Domain-field requirement discharge — domain lanes (**DOMAIN-REFINEMENT-CHAINS** / **DOMAIN-ISSUER-ROUTES**). |
| 8(+68 secondary) | `call … has operational envelope … but acknowledges neither suspension nor blocking; call acknowledgements must match exactly` | Call-acknowledgement family (suspension/blocking contract). |
| 6+6+4 | conversion lowering: `checked trapping conversion`, `signed wrapping conversion requires …`, `Exact integer cast … is not provably representable` | Scalar conversion/cast family — **ARITHMETIC-POLICY-REALIZATION** area. |
| 6+2 | `authored Operator/Call declaration selection occurrence N remained unresolved after successful checking (CheckedOperator/…)` | Declaration-selection residue. |
| 5 | `selected compiler intrinsic … for Terminal boundary named-callable(path(Console::exit_process…` | Console-exit intrinsic selection. |
| 5 | `Lowering(InvalidUnitMachinePlan …)` | Attached-unit machine plan — **GENERAL-CYCLIC-EXECUTION** / **PROVIDER-ATTACHMENT-MACHINE-PLAN** family documented in `known_baseline_failures.md`. |
| 4 | `cannot bound the recast offset …` | Recast-offset bounds — recast_views family. |
| 4 | `Lowering(Unsupported("nested call root has no exact …"))` / `("Unit body omits or duplicates…")` (4) / `("Unit graph has unreachable…")` (2) | Unit-structure lowering families. |
| 3 | FMA: `native artifact native instruction selection failed: FMA provider transport is not implemented in the common instruction pipeline` | Documented FMA-transport fence (TASKS.md:5838 family). |
| 3 | `fixed-array length …: const evaluation … machine … is not build-time admissible` | Const-admissibility for array lengths. |
| 3 | `Lowering(Unsupported("primitive projection requires a structural carrier field"))` | Carrier byte-index/write canaries — primitive-projection lowering. |
| 3 | `Lowering(OperationProofUnavailable(ObligationId(N)))` | Generic trait-default proof obligation — operation-proof discharge. |
| 3+24 secondary | `cannot prove index … is within length …` (SpreadN::evaluate::write_field / slice length) | Index-within-length proofs. |
| ~10 | `failed to resolve /home/ubuntu/repos/Omega/tests/omega/pass/…` (layouts/runtime_plan_laid_*, recast/runtime_*, slices/runtime_*) | Roster drift — roster names fixtures that no longer exist at this revision (fixture-roster respell). |
| 2 | `receiving terminal-authority policy version … does not classify normalized foreign mechanism` / `root-reachable checked physical operation` | **TWO-AXIS-TERMINAL-AUTHORITY-REVIEW** territory. |
| 2 | architecture review rows (`machine-emission` unreviewed physical-pipeline dependency; `resolved-layout-to-resolved-layout` unauthorized dependency) | Architecture-boundary fence — dependency-review rows. |
| remainder | ~30 singleton families (hosted-receiver custody, cyclic receiver execution, EFI handoff mutability, immutability writes, mixed arithmetic domains, ensures-contract exits, callback custody, console-input envelope, …) | Each attributes to the same owning lanes above (entry/borrow/provider/domain/arithmetic families). |

## Negative-fixture obligation

The criterion also requires every negative fixture to reject. The
negative half of this gate is
`proof_and_float_suites::proof_and_domain_canaries::fail_canaries_reject_with_expected_diagnostic_fragment`,
which is itself among the 1178 failures: **10 drifted fail canaries —
8 wording respells and 2 silent acceptances** — byte-for-byte the same
drift inventory witnessed for the RC-DIAGNOSTICS row at `e76d715c` (see
`wiki/drafts/rc_diagnostics_linux_x86_64.md`). The two silent
acceptances are `ownership/linear_ambiguous_state_result_mapping` and
`calls/guarded_value_call_terminal_rejected`; at witness time nine of
the ten fixture directories were fenced to `RC-DIAGNOSTICS-GATE`
(Jarod / swarm-w9-rc-diagnostics-gate). All other negative fixtures
still reject.

## Disposition

The suite is dominated by one upstream regression — the
`attachment_type_identity` / ProgramEntry-establishment rejection —
which alone accounts for 662 of 1178 failures (56%) and gates most
service-bearing positive fixtures across every canary module. Behind it
the red distributes over roughly forty small families, every one
attributed above to a live owned lane (arithmetic-range proofs, entry
exactness, Fused-provider plans, borrow custody, package visibility,
indexing/byte-view lowering, domain discharge, call acknowledgements,
conversion legs, terminal-authority review, and the samples-compile
Windows-entry binding).

This row is a measurement record only — no fixture or source files were
modified. The criterion remains unmet on this host until the owned
families land; the release contract additionally requires all eight
gates on one clean commit across the four required hosts.
