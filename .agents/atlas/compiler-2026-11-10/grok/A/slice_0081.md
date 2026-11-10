# slice_0081 (rows A0618–A0625, area_04 ACCEPT)

Digest preamble: `digests/area_04.md` lines 1–9 (path shorthand: `S03/` = `omega-rust/omega/compilation/pipeline/03_target-operations-to-selected-instructions/src/`; `LRC` = `legalization_reference_custody.rs`; `LSG` = `legalization_scalar_graph.rs`; `LSrc` = `legalization_source.rs`).

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0618 | S03/legalization.rs:240-243 | CLOSED | Validated optimization units cannot carry duplicate `(machine, owner, obligation)` accepted-fact rows. |
| A0619 | S03/legalization_descriptor_inputs.rs:81-91 | REACHABLE | Swallowed boundary-operation lookup lets the reference catalog build proceed; audit custody does not treat that as rejection. |
| A0620 | LRC:6004-6006 (and related) | UNDECIDED | Early return on flow-proved-impossible positions is read; whether a wrong impossibility proof can reach this path is not established. |
| A0621 | LRC:6637 | UNDECIDED | `apply`’s default arm is read; which reference-affecting operation lacks an arm and should fail compilation is not identified. |
| A0622 | LRC:3495-3503, 5598-5617, 5790-5796 | REACHABLE | Custody simulation continues past missing leaves, missing call-result custody, and no-contract discard skips. |
| A0623 | LSG:228-230, 2304-2309, 1849-1853 | REACHABLE | Edge binding, descriptor-origin, and empty-path window checks admit inputs that skip the intended guards. |
| A0624 | LSG:8451-8459, LSrc:691-727 | CLOSED | Stage-01 unit validation already enforces `IntegerArithmetic` operand types before stage 03 omits re-check for `Exact`. |
| A0625 | LSG:8948-8953, 8914-8921 | REACHABLE | `match_input` skips structural call-argument checks on the standalone verify path production legalization does not run. |

## A0618

- **Site (read):** `omega-rust/omega/compilation/pipeline/03_target-operations-to-selected-instructions/src/legalization.rs:234-243` — `accepted_fact` filters `unit.accepted_obligation_facts` and takes `.next()` only; no duplicate rejection at this site.
- **Enclosing function (read):** `accepted_fact` and `facts` in the same module (lines 221–232).
- **Callers (read):** ripgrep `\baccepted_fact\b` over `omega-rust` — 29 files, 283 total matches (definitions plus call sites); production stage-03 call sites include `legalization.rs` (5), `legalization_source.rs` (34), `legalization_scalar_graph.rs` (2), `legalization_target.rs` (2).
- **Upstream trace (read):** `omega-rust/omega/compilation/representations/abstract-operations/optimization-unit/semantics/src/unit_validation.rs:9697-9706` rejects any unit whose `accepted_obligation_facts` are not strictly increasing in `(machine, owner, obligation)`; equal triples fail validation.
- **Closing check (read):** `optimization_unit/semantics/src/unit_validation.rs:9701-9706` — duplicate triples cannot exist on a validated unit passed into legalization (e.g. stage 01 `pass_manager.rs:1313` calls `validate_psi_optimization_unit_with_admitted_cycle_machines`).
- **Verdict:** **CLOSED** — first-match behavior is harmless when the unit index invariant already forbids duplicates.

## A0619

- **Site (read):** `legalization_descriptor_inputs.rs:76-91` — `DescriptorInputs::operation` calls `selected_boundary_operation(...).ok()` and on `Err` substitutes `&node.operation` instead of failing.
- **Enclosing function (read):** `operation` (lines 76–92).
- **Callers (read):** `operation` is passed as a closure into `optimized_reference_catalog_graph_with_boundary_preservation` at `legalization_descriptor_inputs.rs:64-71`; `references()` at 61-73; consumer `stored_calls::prepare` at `LRC:2877-2904`.
- **Trace (read):** If catalog construction succeeds, `block_entry_states` (`LRC:7063+`) runs custody with `stored_calls::prepare(descriptor_inputs)` (`LRC:7144`). If `references()` is `None`, `prepare` returns `Effects` with `reference_catalog_available: false` at `LRC:2902-2903` without error. `verify_target_operation_plan` → `audit_target_operation_plan` (`legalization.rs:483-517`) calls `match_input` but does not re-validate per-node boundary operation resolution.
- **Input (inferred):** A target/abstract/unit triple where `selected_boundary_operation` would error for some catalog node, but substituted abstract operations still yield `Some` catalog graph and audit completes.
- **Verdict:** **REACHABLE** on the standalone verify/audit path (not on `legalize_target_operations` → `seal`, `legalization.rs:405-437`).

## A0620

- **Sites (read):** `LRC:3840-3844` `position_is_impossible`; `LRC:6004-6006` early `Ok(())` in `apply`; `LRC:6853-6855` in `apply_terminator`; `LRC:7209-7210` skip impossible edges; `LRC:7286-7297` install impossible block contexts.
- **Producer (read):** `LRC:1100-1125` records flow/edge feasibility into `observations` via `capture_predicate_feasibility` / `edge_feasibility`.
- **Callers (read):** ripgrep `position_is_impossible` — 3 matches, all in `legalization_reference_custody.rs`.
- **Digest note (read):** area_04 classifies this as open; traced only to the producer, not whether analysis can be wrong.
- **Verdict:** **UNDECIDED** — would need evidence that `FlowFeasibility::ProvedImpossible` can be true at a position that still executes reference-affecting work in a checked program.

## A0621

- **Site (read):** `LRC:6637` — `_ => {}` at end of `apply`’s `match operation` (match begins `LRC:6016`).
- **Enclosing function (read):** `apply` (`LRC:5996-6648`); always runs `custody.validate_occurrences()` at 6640-6647 after the match.
- **Callers (read):** `apply` invoked from custody worklist at `LRC:7191`; `apply_terminator` at `LRC:7189`.
- **Trace (read):** Operations with explicit arms update `custody.leaves` (e.g. `EstablishReference`, `CallStructural`, `BoundaryCall`); unmatched operations perform no custody mutation but `apply` still returns `Ok(())` unless `validate_occurrences` fails.
- **Verdict:** **UNDECIDED** — no specific `AbstractOperation` variant was identified that both falls through `_ => {}` and should cause legalization rejection solely for missing custody arm.

## A0622

- **Site 1 (read):** `LRC:3495-3503` inside `stored_calls::stage` effect loop — `custody.leaves.get(&source)` `None` with a case segment in the path executes `continue` instead of `Err`.
- **Site 2 (read):** `LRC:5598-5617` in call-reference transition — missing moved/copied leaf executes `continue` at 5616 when `transfers_existing`.
- **Site 3 (read):** `LRC:5790-5791` `discard_subtree` — `source_contract` missing returns `Ok(false)`; `discard_owned` (`LRC:5772-5779`) propagates that success; cleanup path `LRC:7040` uses `?` so no discard occurs.
- **Trace (read):** `block_entry_states` / custody worklist (`LRC:7180-7198`) drives `apply` and terminator handling; failures aggregate at `LRC:7282-7283`.
- **Input (inferred):** Programs where stored-call effects reference inactive-case sources without tracked leaves, call-result paths lack custody records, or owned cleanup targets places with no contract still traverse custody simulation without those sites erroring.
- **Verdict:** **REACHABLE** for the reference-custody audit simulation (`legalization_target.rs:1244` calls `block_entry_states`).

## A0623

- **Site 1 (read):** `LSG:213-230` `address_joins::edge_bindings` — binding to a parameter that is not an address join returns `true` without `descriptor_argument_accessible` / projection checks (lines 228-230).
- **Callers (read):** `legalization_target.rs:1684-1690`, `1744-1750` (terminator matching in LT audit).
- **Site 2 (read):** `LSG:2304-2318` — `CallDescriptorOrigins::resolve` returns `Ok(Vec::new())` for nonlinear multiplicity when `owned_slots` is empty.
- **Site 3 (read):** `LSG:1849-1853` in `borrowed_windows::extent` — empty path authenticates `MutableBorrow` window requests (or parameter roots) without backing-root verification.
- **Verdict:** **REACHABLE** — non-join edge bindings pass LT successor checks; empty descriptor-origin and empty-path window cases skip stricter guards.

## A0624

- **Site 1 (read):** `LSG:8444-8460` inside `nodes::validate` (`LSG:7763+`) — for `IntegerArithmetic`, operand `value_type` checks are skipped when `arithmetic.policy() == Exact` (condition at 8451).
- **Site 2 (read):** `LSrc:691-726` `project_exact_integer_arithmetic` — requires `accepted_fact` and `references_obligation` but does not compare operand value types to `scalar_type`.
- **Closing check (read):** `unit_validation.rs:12165-12186` validates `IntegerArithmetic` operands (exact add/sub via `admits_exact_*`; other cases via `binary(*left, *right, *scalar_type)`).
- **Pipeline (read):** Stage 01 `pass_manager.rs:1313` validates the unit before later stages; stage 03 production uses `legalize_target_operations` with the same unit (`optimized.rs:98-106`).
- **Verdict:** **CLOSED** for the normal pipeline — stage 03 omits redundant operand typing when the unit is already validated.

## A0625

- **Sites (read):** `LSG:8914-8967` inside `match_input` — `CallStructural` with unit/scalar result still enters the block; structural args whose path contains `Referent` hit `continue` at 8951-8952 and skip `structural_call::argument_at`.
- **Enclosing function (read):** `match_input` (`LSG:8688-8970`).
- **Callers (read):** ripgrep `\bmatch_input\b` — 2 matches: definition and `legalization.rs:509` inside `audit_target_operation_plan` only.
- **Production path (read):** `legalize_target_operations` (`legalization.rs:405-437`) does not call `match_input`; only `verify_target_operation_plan` does (`legalization.rs:466-517`).
- **Input (inferred):** A verify/audit input where structural call arguments crossing a referent are ill-typed or ill-custodied but other `match_input` checks pass.
- **Verdict:** **REACHABLE** on the standalone verify path.

## What I did not read

- Full body of `LRC::apply` match (all arms between `LRC:6016-6636`).
- Complete `descriptor_results::catalog_graph` fixed-point logic beyond `descriptor_results.rs:1067-1124`.
- Whether every compilation entry point always supplies a unit that passed `validate_psi_optimization_unit` immediately before stage 03 (only stage 01 pass-manager call site confirmed).
- Corpus or sample programs witnessing REACHABLE cases for A0619, A0622, A0623, A0625.

SLICE COMPLETE
