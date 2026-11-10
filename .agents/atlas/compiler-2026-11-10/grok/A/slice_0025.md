# slice_0025

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0184 | attached_unit.rs:15256-15268 | REACHABLE | `Ok(false)` from `prepare_immutable_local_borrow` is dropped via `?` and callers still finish lowering the borrow path. |
| A0185 | attached_unit.rs:11984-11992 | CLOSED | `lower_step` skips deferred release here; the same `plan.operations` are retired at exit in `lower_reference_completion_regions`. |
| A0186 | attached_unit.rs:11666-11670 | UNDECIDED | Loop `break` on first `External` call limits pre-establishment; whether later literals are established elsewhere on the ordinary-machine path is not determined. |
| A0187 | attached_unit.rs:8792-8802 | UNDECIDED | Scoped miss falls back to an unscoped `(expression, invalid scope)` row; whether that can pick the wrong application for a satisfiable checked program is not determined. |
| A0188 | attached_unit.rs:12242-12244 | UNDECIDED | Quotient rows with no matching `type_ids` entry are omitted from the catalog; effect on acceptance vs incomplete catalog is not determined. |
| A0189 | attached_unit.rs:4491-4531 | CLOSED | `continue` arms only shrink `qualification_establishments`; they do not bypass errors or widen what lowering accepts. |
| A0190 | emission.rs:1355-1357 | UNDECIDED | Empty `owned_transfer_paths` match yields `Ok(Vec::new())`; whether checker always supplies rows for operands that must carry claims was not traced. |
| A0191 | emission.rs:436-444 | CLOSED | Filter excludes non-matching bindings and `selected == remaining`; callers treat an empty destination list as no reservations to make. |

## A0184

Digest entry 9 (`area_10.md`) cites `attached_unit.rs:15256-15268` with call site `15227`. **read** at `omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi/src/attached_unit.rs:15243-15352` (`prepare_immutable_local_borrow` in full) and `15236-15240` (`prepare_borrowed_argument`).

Early exits return `Ok(false)` when `root` is not `PlaceRoot::Symbol` (`15256-15257`), the local is absent (`15260-15261`), the type is not primitive (`15263-15264`), or the local is mutable (`15266-15267`). Success returns `Ok(true)` at `15351`.

At `15227-15238`, `prepare_borrowed_argument` calls `prepare_immutable_local_borrow(...)?`. The `?` operator propagates only `Err`; an `Ok(false)` is discarded and the function returns `Ok(())` at `15240`.

**Caller search** `\bprepare_immutable_local_borrow\b` in `omega-rust`: 4 matches — 1 definition (`attached_unit.rs:15243`), 3 call sites (`attached_unit.rs:15227`, `unit_arguments.rs:2079`, `unit_operation_frame.rs:7930`). Digest’s “single call site” does not match the search; the cited `15227` site is one of three.

**`\bprepare_borrowed_argument\b`**: 5 matches — 1 definition (`attached_unit.rs:15136`), 4 call sites (`unit_operation_frame.rs:1069`, `1723`, `8023`; `unit_structural_values.rs:4263`; `unit_arguments.rs:2058`).

Trace upward from `unit_operation_frame.rs:7930-7969` (**read**): on `Ok(false)`, lowering still runs `mint_runtime_index_selectors` and `lower_reference_source` and publishes reference storage (`7971-7972`).

**REACHABLE** (read): A checked body may schedule a shared borrow of a structural argument whose `structural_storage_root()` is not a symbol local (or matches the `Ok(false)` guards at `15256-15268`) while stage 06 still completes `prepare_borrowed_argument` / reference lowering without materializing the immutable local home that `prepare_immutable_local_borrow` would have established on `Ok(true)`. Whether stage 05 always rejects such a body was not traced here.

## A0185

Digest entry 10. **read** `attached_unit.rs:11764-11992` (`MachineEmission::lower_step` through the `ReleaseReference` guard) and `11984-11991` (match on `defer_to_exit: true` then `return Ok(())`).

**Caller search** `lower_step`: 2 matches — definition `attached_unit.rs:11764`, call `attached_unit.rs:10795` inside `ordinary_machine::emit` schedule loop (`10792-10798`).

**read** exit path: `ordinary_machine::emit` later calls `completion_cleanup::scalar_returns` at `10910-10928` with `NormalExitSource { operations: &plan.operations, ... }`. That reaches `for_planned_actions` at `5942-6032`, which calls `unit::unit_cleanup::lower_reference_completion_regions` at `6032-6057`.

**read** `unit.rs:2221-2247` (`lower_reference_completion_regions`): iterates `planned_operations`, selects `ReleaseReference { defer_to_exit: true, ... }` (`2237-2244`), and builds cleanup carriers (continues through `2248+`).

**CLOSED** (read): Every deferred `ReleaseReference` skipped during the argument schedule at `11984-11991` is still present in `plan.operations` passed into exit cleanup at `10921` and processed at `unit.rs:2237-2247` via `attached_unit.rs:6032`. Composed-control also has defer-to-exit handling at `unit_composed_control.rs:12559-12582` (**read** listing only).

## A0186

Digest entry 11. **read** `establish_call_literals` in full at `attached_unit.rs:11637-11761`, especially `11654-11671` (collect literal arguments from calls, `break` when `BoundaryCompletion::External(_)` at `11666-11670`).

**Caller search** `\bestablish_call_literals\b` in `omega-rust`: 2 matches — definition `11637`, single call `10595` in `ordinary_machine::emit` before the schedule walk.

Trace: `10595-10609` pre-establishes only literals gathered before the `break`. Later operations’ call literals are not added to `literal_arguments`.

**UNDECIDED**: Other `emit_call_literal` / `byte_sequence_literal` sites exist (`unit_structural_values.rs:6943`, `unit_composed_control.rs:1200`, etc. — **read** grep listing, not each path). Whether a literal on a call after the first `External` completion on the ordinary-machine path is always established later, or can complete lowering without it, was not determined.

## A0187

Digest entry 12 cites `8792-8802` and `8827-8833`. **read** `SourceScalarApplications::expression_in` at `8783-8803` and `recipe_in` at `8818-8834` (same fallback pattern with `!scope.is_valid()`).

**Caller search** `\bexpression_in\(` in `omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi`: 2 call sites — `attached_unit.rs:8599` (inside `retain_expression_in`), `scalar_computations.rs:7578`.

**`\bretain_expression_in\b`**: 6 matches in `attached_unit.rs` — definition `8593`, uses `7689`, `7708`, `8474`, `8497`, `8536`.

Trace at `8593-8609` (**read**): `retain_expression_in` calls `expression_in` first; on miss, checks constant evaluation and other retention paths before inserting a new application.

**UNDECIDED**: Fallback at `8792-8797` returns the first unscoped row for the same `expression` when the `(expression, owner)` row is absent. Whether checked facts guarantee that unscoped application equals the scoped one for every reachable `(expression, owner)` was not read.

## A0188

Digest entry 13. **read** `quotients::retain` at `attached_unit.rs:12215-12265`, skip at `12242-12244` (`continue` when `type_ids` has no matching identity).

**Caller search** `quotients::retain`: 1 production call `attached_unit.rs:17200` in `lower_unit_closure` retention (**read** `17192-17214`). Loop at `17210-17214` errors if a catalog representation lacks a structural type declaration; skipped quotients never enter the catalog.

**UNDECIDED**: Whether a quotient definition can be present in checked trees with a missing `type_ids` row while lowering should still reject the program, versus silently shipping a module without that quotient representation, was not determined (stage 05 producer of `type_ids` not traced).

## A0189

Digest entry 14. **read** `call_result_qualification_establishments` at `attached_unit.rs:4455-4541`; `continue` at missing domain plan (`4491-4493`), empty routes (`4495-4496`), not established at call ensures (`4509-4510`), missing trait signature (`4514-4516`), and no matching route in catalog (`4526-4538` uses `if let Some(index)` only).

**Caller search** `\bcall_result_qualification_establishments\b`: 2 matches — definition `4455`, call `1765` when lowering call result value with valid issuer (**read** `1763-1774`).

**CLOSED** (read): Each `continue` omits one optional `ResultQualificationEstablishment` entry. Failures still return `Err` from `lower_establishment_routes`, `lookup_domain_id`, or unsupported paths elsewhere in the function. Shrinking `value.qualification_establishments` does not skip a rejecting check on the same path; it records fewer establishment routes in the lowered result.

## A0190

Digest entry 15. **read** `lower_owned_operand_claims` at `emission.rs:1320-1412`; early return `1355-1357` when `evaluated_transfers` is empty after filtering `owned_transfer_paths` (`1343-1354`).

**Caller search** `\blower_owned_operand_claims\b` in `omega-rust`: 4 matches — definition `emission.rs:1320`, calls `unit_structural_values.rs:8008`, `emission_cleanup.rs:2898`, `attached_unit.rs:5158`.

Trace from `unit_structural_values.rs:8005-8023` (**read**): for non-`Unrestricted` multiplicity, empty claims from `lower_owned_operand_claims` become `CompletedOperand::checked(argument, claims)` with an empty `claims` vector.

**UNDECIDED** (digest: producer not traced): Whether stage 05 always inserts `owned_transfer_paths` rows whenever an owned operand must carry consume claims was not traced. If rows can be absent for a body that should be rejected, lowering would accept the operand with no claims at `1355-1357`.

## A0191

Digest entry 16. **read** `emission::claim_bindings::selections::destinations` at `emission.rs:426-447`; filters at `436-443` skip wrong coordinates and `binding.selected == binding.remaining`.

**Caller search** `selections::destinations` / `destinations(`: `reserve` at `418-419`, direct calls in `scalar_computations.rs` at `2488`, `2734`, `5947` (**read** `2485-2496` for one site).

Trace: `reserve` (`409-423`) collects destinations then reserves each claim; an empty vector reserves nothing and returns `Ok(destinations)`.

**CLOSED** (read): `selected == remaining` marks a binding with no effective selection delta, so excluding it avoids reserving a non-destination. Call sites use the returned list as the exact set of claim destinations; empty list means no claim edges for that decision, not a bypass of later unsupported checks in `edge_bindings` / dispatch lowering.

## What I did not read

- Stage 05 checked-tree producers for: immutable local borrow eligibility (`prepare_immutable_local_borrow` inputs), `owned_transfer_paths` population, `type_ids` completeness for quotients, and scoped vs unscoped scalar application invariants.
- Full `retain_expression_in` fallthrough after `8611` for every definition kind.
- Whether post-`External` call literals on the ordinary-machine path are always covered by `unit_operation_frame` / `structural_byte_sequence_store` emission beyond the grep listing.
- `git show 457adf4a11f` line-by-line diff; cited sites matched current paths on read.

SLICE COMPLETE
