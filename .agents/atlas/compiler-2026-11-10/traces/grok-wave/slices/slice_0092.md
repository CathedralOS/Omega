slice_0092

| entry id | file:line | verdict | one-line reason |
|----------|-----------|---------|-----------------|
| A0704 | unit_validation.rs:3773-3816 | REACHABLE | `ByteSequenceWrite` skips predicate/UTF8 checks when `byte_write_predicates` is `None`. |
| A0705 | unit_validation.rs:3750-3771 | REACHABLE | `WriteOnlyPrimitiveStore` ASCII freshness runs only when `requires_ascii == Some(true)`. |
| A0706 | unit_validation.rs:4479 | REACHABLE | `may_change_owned_source` default arm is `false`; `ElementViewWrite` is not listed. |
| A0707 | unit_validation.rs:5455-5457 | REACHABLE | `holds` establishment arm trusts `declared_ascii` on a structural result at the root. |
| A0708 | unit_validation.rs:5793 | REACHABLE | `ControlCycle` is not emitted when `admit_cycles` is true for that machine. |
| A0709 | unit_validation.rs:6724-6727 | REACHABLE | Missing producer map entry skips availability checking via `continue`. |
| A0710 | unit_validation.rs:8506 | UNDECIDED | Did not enumerate every operation variant that hits `_ => {}`. |
| A0711 | unit_validation.rs:7517-7550 | REACHABLE | `ByteSequenceEqual` / `ByteSequencePredicate` only validate `Whole` operands. |

### A0704

1. Read `byte_predicates::validate_indexed_byte_stores` in full (`unit_validation.rs:3736-3894`, read).
2. At `unit_validation.rs:3773-3816` (read), `Operation::ByteSequenceWrite` enters `if let Some(predicates) = byte_write_predicates(...)`; when `byte_write_predicates` returns `None`, the whole predicate block is skipped and the loop `continue`s with no error (`unit_validation.rs:3815-3816`, read).
3. Read `byte_write_predicates` (`unit_validation.rs:3724-3734`, read) and `owned_byte_predicates` (`unit_validation.rs:3897-3935`, read): `None` when the place is not an arrival parameter and owned derivation fails (not a concatenation result, wrong multiplicity/claims/qualifications, or type not a bounded owned byte sequence, `unit_validation.rs:3919-3928`, read).
4. Search `rg 'validate_indexed_byte_stores' omega-rust`: 2 hits (definition `unit_validation.rs:3736`, sole production call `unit_validation.rs:9638`, read).
5. Call chain read: `validate_indexed_byte_stores` ← `validate_function` (`unit_validation.rs:9638`) ← `index_and_validate_unit_catalogs` (`unit_validation.rs:1233`) ← `validate_psi_optimization_unit_with_control_cycles` (`unit_validation.rs:18265`) ← `validate_psi_optimization_unit` (`unit_validation.rs:18207-18210`, read). Failure becomes `Err(OptimizationUnitValidationError::...)`, rejecting the unit.
6. Input reaching the accepting branch: a `ByteSequenceWrite` node whose `destination` place yields `None` from `byte_write_predicates` (inferred: non-parameter owned byte destination outside the bounded-concatenation predicate model).
7. **Verdict: REACHABLE** — A unit whose function contains a `ByteSequenceWrite` to such a destination, where the store would fail the UTF8-vs-ASCII split check if predicates were computable, should be rejected but is accepted here because no `InvalidByteSequenceWrite` is returned.
8. Unsure: whether any producer path can emit that shape without already failing an earlier gate (not traced through lowering).

### A0705

1. Read `validate_indexed_byte_stores` (`unit_validation.rs:3736-3894`, read) and `fixed_byte_freshness::requires_ascii` (`unit_validation.rs:5388-5416`, read).
2. At `unit_validation.rs:3750-3771` (read), `WriteOnlyPrimitiveStore` runs ASCII `holds` only when `requires_ascii(unit, destination, path) == Some(true)`; `None` or `Some(false)` skips the check and hits `continue` (`unit_validation.rs:3771`, read). `requires_ascii` returns `None` when `primitive_store_byte_predicates` returns `None` (`unit_validation.rs:5403-5411` `?`, read).
3. Search `rg 'validate_indexed_byte_stores' omega-rust`: 2 hits (same chain as A0704).
4. Same acceptance path through `validate_psi_optimization_unit` (read as A0704).
5. Input: `WriteOnlyPrimitiveStore` whose destination/path makes `requires_ascii` `None` or not `Some(true)` (inferred: byte predicates not computable for that primitive-store destination).
6. **Verdict: REACHABLE** — A unit with a fixed indexed byte store that needs ASCII freshness but whose destination predicates cannot be computed should be rejected; here the store is accepted with no ASCII `holds` call.
7. Unsure: none for the branch logic; producer reachability not fully traced.

### A0706

1. Read `may_change_owned_source` in full (`unit_validation.rs:4451-4481`, read) and enclosing `descriptor_source_predicates` tail (`unit_validation.rs:4413-4446`, read).
2. At `unit_validation.rs:4479` (read), `_ => false` — `AbstractOperation::ElementViewWrite` is not matched (contrast `holds` invalidation list including `ElementViewWrite` at `unit_validation.rs:5529`, read).
3. Search `rg 'may_change_owned_source' omega-rust`: 6 hits — `unit_validation.rs` (definition plus two call sites at `unit_validation.rs:4421`, `4419-4423`, read); `terminal-psi/semantics/src/bytes.rs` (3, not read).
4. Read caller use at `unit_validation.rs:4419-4423` (read): when `may_change_owned_source` is false for all nodes, concatenation operand analysis returns `Some(declared)` without requiring operands to be present.
5. Input: concatenation (or descriptor predicate) analysis where an `ElementViewWrite` to the owned `source` place would change bytes but `may_change_owned_source` returns `false`.
6. **Verdict: REACHABLE** — A unit where an owned byte source is mutated by `ElementViewWrite` before a predicate-bearing use should be treated as stale; this helper reports no change, so analysis can accept declared predicates.
7. Unsure: end-to-end whether such a unit survives all of `validate_function` (only traced through predicate helper, not full ownership replay).

### A0707

1. Read `fixed_byte_freshness::holds` (`unit_validation.rs:5418-5537`, read) and `declared_ascii` (`unit_validation.rs:5552-5588`, read).
2. At `unit_validation.rs:5455-5457` (read), the establishment closure returns `true` when `operation.structural_result()` is `Some` and `declared_ascii(unit, result.structural_type, &result.qualifications, &result.projected_qualifications, path)` — trusting declared domains/projections at `path`, not evidence of a prior establishing operation.
3. Search `rg 'requires_ascii\('` in optimization-unit-semantics: 3 hits, all `unit_validation.rs` (read subset above).
4. Read `validate_indexed_byte_stores` caller chain (same as A0704) — ASCII establishment for stores depends on `holds`.
5. Input: `holds` query at use site where the only matching establishment is a structural result at the root with AsciiOnly declared via qualifications but no real ASCII-establishing producer on all paths.
6. **Verdict: REACHABLE** — A unit whose operation result falsely declares ASCII qualification at the root can satisfy `holds` and allow a downstream ASCII-sensitive store that should fail freshness checking.
7. Unsure: whether catalog/result metadata rules always prevent inconsistent declarations (not read in `operation_contracts`).

### A0708

1. Read `control_flow::validate_total_cfg` (`unit_validation.rs:5745-5803`, read) and `ControlCyclePolicy` (`unit_validation.rs:5614-5625`, read).
2. At `unit_validation.rs:5793-5801` (read), if topological visit count `visited != blocks.len()` and `!admit_cycles`, emit `ControlCycle`; if `admit_cycles` is true, return `Ok(())` with no cycle error.
3. Read `validate_function` (`unit_validation.rs:9595-9600`, read): `admit_cycles = cycle_policy.admits(function.machine)`.
4. Search `rg 'validate_psi_optimization_unit_with_admitted_cycle_machines' omega-rust`: 4 hits — `unit_validation.rs:18229`, `lib.rs` re-export (2), `pass_manager.rs:1313`, `artifact_admission.rs:1126` (test helper, read). Search `rg 'validate_total_cfg' omega-rust`: 2 hits.
5. Read `pass_manager.rs:1298-1316` (read): admitted machines taken from `ValidatedOptimizerCycleComponents` after `validate_verified_psi_context` / `validate_exact_ranked_cycles` (`validation.rs:44`, read name only, not ranked_cycles body). Read `validate_psi_optimization_unit` default (`unit_validation.rs:18204-18210`, read): default policy admits no machines.
6. Input: any function on a `MachineId` present in `ControlCyclePolicy.admitted_machines` when the CFG has a cycle (`admit_cycles == true` at `unit_validation.rs:5749`, read).
7. **Verdict: REACHABLE** — Callers of the public `validate_psi_optimization_unit_with_admitted_cycle_machines` can admit a machine id with a cyclic CFG; validation accepts the unit without `ControlCycle` even though a cyclic control graph should be rejected for non-admitted machines.
8. Unsure: ranked-cycle validation guarantees for optimizer session admission (read only `validation.rs:18-48`, not full ranked_cycles module).

### A0709

1. Read `validate_structural_place_availability` in full (`unit_validation.rs:6681-6746`, read).
2. At `unit_validation.rs:6724-6727` (read), for each `place` in `operation_place_inputs(&node.operation)`, if `producers.get(&place)` is `None`, `continue` — no `StructuralPlaceNotAvailable` error.
3. Search `rg 'validate_structural_place_availability' omega-rust`: 3 hits (definition, re-export, call `unit_validation.rs:9605`, read).
4. Read `validate_function` order (`unit_validation.rs:9605`, read then `9659 validate_places_and_claims`, read): availability runs before place/claim validation; skipped producer entries are not repaired later in this function.
5. Read `operation_place_inputs` (`unit_validation.rs:6917-6922`, read): unions `operation_place_accesses` and cleanup roster places.
6. Input: any operation at a node listing a `PlaceId` in `operation_place_inputs` that was never inserted into `producers` (no block parameter entry, no structural result producer row).
7. **Verdict: REACHABLE** — A unit where an operation reads a structural place nothing in the function produces should be rejected; here that use is skipped and validation may return `Ok(())`.
8. Unsure: whether `reconstruct_declared_places` / `validate_operation_places` always forces such places into `producers` before this runs (read `validate_places_and_claims` at `2684-2737` only partially).

### A0710

1. Read `validate_structural_root_operations` from signature through the match (`unit_validation.rs:7190-8511`, read; preamble checks `7301-7323` read for all nodes).
2. At `unit_validation.rs:8506` (read), `_ => {}` — no op-specific structural-root check for unmatched variants after shared preamble.
3. Search: no separate symbol; accepting site is the default arm at `8506`.
4. Read callers: only via `validate_function` `unit_validation.rs:9610` (read). Also read companion validators in same function (`9625-9658`, read) that run for all operations regardless of match arm.
5. Input: any `AbstractOperation` variant that reaches the loop, passes `view_memberships_match` / `primitive_locals::validate`, and is not listed in the `match`.
6. **Verdict: UNDECIDED** — Did not enumerate all `AbstractOperation` variants or prove each is covered elsewhere; cannot name a smallest rejecting program without that list.
7. Unsure: complete inventory of fall-through ops and whether a dedicated check exists only outside this `match`.

### A0711

1. Read `ByteSequenceEqual` / `ByteSequencePredicate` arms in full (`unit_validation.rs:7517-7550`, read).
2. Only `ByteSequenceOperand::Whole { source, length }` is validated via `valid_whole`; `Field` and other operand shapes are not matched (read). Same filtering in `descriptor_sources::validate_whole_freshness` (`unit_validation.rs:5306-5311`, read).
3. Search `rg 'ByteSequenceOperand::Field' optimization-unit/semantics`: 3 hits — concatenation validation (`2817`, read) and predicate helpers (`3957`, `4432`, not fully read); no Field handling in the Equal/Predicate arms above.
4. Caller chain: `validate_structural_root_operations` ← `validate_function` `9610` (read).
5. Input: `ByteSequenceEqual` or `ByteSequencePredicate` with a `Field` operand (inferred: references `source.place` / `field` / `length` per `ByteSequenceOperand::Field` at `2817-2821`, read).
6. **Verdict: REACHABLE** — A unit with a byte equal or predicate on a `Field` operand pointing at a non-existent or erased field should be rejected; these arms never inspect `Field`, so the operation is accepted if preamble checks pass.
7. Unsure: whether operand shapes other than `Whole` are producer-forbidden (not traced through lowering).

### What I did not read

- Full `descriptor_source_predicates` (`unit_validation.rs` before `4413`, inferred from partial read).
- `ranked_cycles_validation.rs` body (only `validation.rs:33-48`, read).
- `terminal-psi/semantics/src/bytes.rs` `may_change_owned_source` copies (6-hit grep, not opened).
- Complete `validate_operation_places` match (`2468+`, read only through `EstablishByteSequenceConcatenation` arm).
- Every `AbstractOperation` variant for A0710 fall-through analysis.
- Commit `457adf4a11f` diff; line numbers taken from current tree (re-located; cites align with digest UV lines for cited regions).

SLICE COMPLETE
