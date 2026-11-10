# slice_0143

Area digest preamble (read): `digests/area_15.md` lines 1–9 — Psi foundation, checked interpreter (`CI/` = `omega-rust/psi/interpreter/checked-interpreter/src/`), terminal interpreter (`TI/` = `omega-rust/psi/interpreter/terminal-interpreter/src/`); digest commit `457adf4a11f`, cited line ranges matched in current checkout at the paths shown below (read).

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A1097 | CI/program_values.rs:845-891 | REACHABLE | Coercion pass-through and `Ok(None)` recast fallback can store or present the wrong scalar or reference shape without trapping. |
| A1098 | TI/boundary_effects.rs:51-53 | CLOSED | Empty `scalar_requires` skips only scalar predicates; structural `boundary.requires` are enforced earlier on the same call path. |
| A1099 | TI/boundary_effects.rs:2304-2306 | REACHABLE | View paths append to the referent with no bounds check, and opaque host results keep whatever `opaque_identity` the host supplied. |
| A1100 | TI/boundary_effects.rs:3276-3296 | REACHABLE | Qualification and restoration checks no-op for several mutable host scalar endpoints and when no live owner matches the target place. |
| A1101 | TI/boundary_effects.rs:3562-3574 | REACHABLE | Trait default `poll_*` returns ready/computed unless a handler overrides, and minimal handlers (e.g. test `Sink`) use the default. |
| A1102 | TI/boundary_inputs.rs:1449 | CLOSED | `Ok(None)` defers to later byte-argument preparers or errors; overlap/out-of-range indices are rejected when scalars are bound. |
| A1103 | TI/boundary_storage_inputs.rs:285 | REACHABLE | `direct_original` only records provenance; shared-borrow outputs skip normal-qualification preservation and opaque leaves skip child capture. |
| A1104 | TI/boundary_values.rs:175-181 | CLOSED | Missing entry scalar checks count as complete by design; result custody walks a fixed table; aggregate planning skips duplicate type ids without comparing declarations. |

## A1097

**Cited sites (read):** `omega-rust/psi/interpreter/checked-interpreter/src/program_values.rs:839-891` — `coerce_scalar_with` applies `apply_arithmetic_domain` only for `Value::Int` and F32 conversion for `Value::Float` when `primitive == F32`; `_ => Ok(value)` at `873` leaves other value shapes unchanged. `coerce_scalar_value` returns `Ok(value)` when `primitive_type_reference` is `None` at `889`. `252-256`, `423`: `reference_recast_initializer` and `reference_storage_path` return `Ok(None)` for unrecognised shapes. `2326`: binary `expression_scalar_type` uses `left.or(right)` when only one side has a type. `815-837`: `assignment_target_coercion` uses `expression_type_reference(...) ?` so a missing type yields `None` (no coercion).

**Enclosing functions (read):** `coerce_scalar_with` / `coerce_scalar_value` (`839-891`); `reference_recast_initializer` (`247-356`); `reference_storage_path` (`358-425`); `expression_scalar_type` binary arm (`2310-2327`); `assignment_target_coercion` (`815-837`).

**Caller search (read):** `\bcoerce_scalar_with\b` over `omega-rust` — 4 matches in `program_values.rs`, 3 in `execution.rs` (7 total, read). `\bcoerce_scalar_value\b` — 3 files (`program_values.rs`, `execution.rs`, `execution_expressions.rs`, read). `\breference_recast_initializer\b` — 4 matches, all in `program_values.rs` (read).

**Upward trace (read):** `execution.rs:59-177` — assignment applies `assignment_target_coercion` unless `reference_shell` is set (`60-63`); when coercion is `None`, the evaluated value is stored unchanged (`173-177`). `execution_scalars.rs:347-354` — for reference-typed destinations, if `capture_reference_view` returns `Ok(None)`, evaluation falls through to `eval_read_cell` and clones the cell instead of building a `Value::Ref`. `reference_recast_initializer` `Ok(None)` at `345-346` feeds that path via `capture_reference_view` (`230-234`). Local initializer binding uses `coerce_scalar_value` at `execution.rs:1842` without a separate type-equality trap after coercion.

**Input:** Build-time evaluation of a checked tree where a scalar assignment or initializer pairs mismatched primitive carriers (e.g. boolean stored into integer-typed local) or a reference expression whose recast shape is not recognised.

**Smallest program (plain words):** An assignment of a boolean literal into a narrower integer local without a recast the initializer recognises; coercion leaves the boolean bits in place and the interpreter accepts the store. Alternatively, a reference-typed expression whose recast chain returns none is read as a plain cell value instead of a reference view.

## A1098

**Cited site (read):** `omega-rust/psi/interpreter/terminal-interpreter/src/boundary_effects.rs:51-52` — `validate_input_requirements` returns `Ok(())` when `boundary.scalar_requires.is_empty()`.

**Enclosing function (read):** `validate_input_requirements` (`43-119`); when non-empty, predicates are evaluated via `InputObservations::evaluate` (`101-118`).

**Caller search (read):** `\bvalidate_input_requirements\b` over `omega-rust` — 1 definition (`boundary_effects.rs`), 1 call (`engine_calls.rs:3383`, read).

**Upward trace (read):** `engine_calls.rs:3380-3389` — before `validate_input_requirements`, `validate_boundary_requirements` (`engine_custody.rs:2414-2436`) checks every `boundary.requires` structural qualification on arguments (`2418-2434`). Scalar parameter type and `validate()` are still checked when `scalar_requires` is non-empty (`58-66`); empty list only skips the proposition loop (`101-118`). Intrinsic normal qualifications are treated as covered at `1996-1998` and `2025-2027` in preservation helpers (read, callee bodies not fully traced).

**Input:** Host boundary invocation with empty `scalar_requires` but non-empty structural `requires` on arguments.

**Verdict basis:** Structural requirements are not skipped; only the scalar proposition list is absent, so this branch is harmless when admission already omitted scalar contracts.

## A1099

**Cited sites (read):** `boundary_effects.rs:2294-2307` — `TerminalOriginalStorageLoan::scalar_path` without `scalar_projection` extends `referent` with `view_path` unchanged. `2754-2758` — opaque host structural results store `value.opaque_identity` in `structural_host_atoms` without an origin check at this site.

**Enclosing items (read):** `scalar_path` on `TerminalOriginalStorageLoan`; `publish_effect_result` match arm for opaque structural scalars (`2737-2762`).

**Caller search (read):** `scalar_path` method — used from lowering (`scalar_computations.rs:3484`, `3529`, read) and host runtime (`original_homes.rs:473`, `542`, read); definition in `boundary_effects.rs` (read).

**Upward trace (read):** Host effect completion accepts any structural value whose declaration matches multiplicity/qualifications and passes `!requires_materialized_host_custody` (`2746-2752`), then pins the opaque identity (`2754-2758`). View-byte overlay paths that rely on `scalar_path` without a projection table inherit unvalidated suffix concatenation.

**Input:** Admitted Terminal Psi program whose host effect returns an opaque structural value or whose byte overlay mapping uses a loan without `scalar_projection`.

**Smallest program (plain words):** A host boundary effect that returns an opaque structural token with an attacker-chosen identity handle; the interpreter stores that identity and later treats paths built by appending view segments as valid loan targets.

## A1100

**Cited sites (read):** `boundary_effects.rs:3276-3295` — `validate_scalar_loan_qualifications` / `validate_scalar_loan_restoration` return `Ok(())` when `scalar_qualification_owner` is `None`. `3487-3504` — `OwnedArrayElement`, `RepresentationByte`, and `ViewByte` endpoints yield `None` owner. `3418-3422` — `captured_scalar_restriction` returns `Ok(None)` for non-`ScalarField` endpoints. `3339-3395` — restoration requires a live `owns_target` value; if none match, loops add no failures and return `Ok(())`.

**Enclosing functions (read):** `validate_storage_loan_restoration_at` (`3313-3395`); `captured_scalar_restriction` (`3398-3449+`); `scalar_qualification_owner` (`3487-3504`).

**Caller search (read):** `validate_scalar_loan_qualifications` called from `boundary_effects.rs:3141` (`capture_storage`) and `boundary_storage_inputs.rs:487`, `668` (read).

**Upward trace (read):** Mutable boundary capture calls `validate_scalar_loan_qualifications` before reading storage (`3139-3142`), but array-element writes during byte-buffer commit use `PrimitiveStorageEndpoint::ArrayElement` (`480-487`) which does have an owner (`3493`), while representation/view/owned-array-element endpoints skip qualification entirely (`3501-3503`). Restoration after mutable host replacement can succeed with no owning structural value in the scanned machines/references.

**Input:** Mutable host loan targeting a representation byte, view byte, or owned array element, or replacement storage whose endpoint is not `ScalarField`.

**Smallest program (plain words):** A boundary storage commit that mutates a view-byte or representation-byte slot under a mutable host loan; qualification coverage is not checked and restoration accepts the write without proving an original owner still covers the place.

## A1101

**Cited sites (read):** `boundary_effects.rs:3562-3574` — default `TerminalEffectHandler::poll_host_computation` and `poll_effect_readiness` return `Ok(true)`.

**Enclosing trait (read):** `TerminalEffectHandler` (`3555-3584+`).

**Caller search (read):** `\bpoll_host_computation\b` over `omega-rust` — trait default plus overrides in `embedding.rs`, `engine_calls.rs` caller, build-system adapters, host imports, and tests (read listing: 9 files).

**Upward trace (read):** `engine_calls.rs:3474-3500` — after `poll_effect_readiness`, `poll_host_computation` gate-keeps `HostComputationPending`; default `true` means computation is treated as finished. `embedding.rs:1788-1792` forwards to adapters when present. `TerminalEffectHandler for Sink` (`terminal_carried_requirements.rs:28-44`) does not override `poll_host_computation` (read), so tests using `Sink` inherit `Ok(true)`.

**Input:** Boundary call suspended for host computation with a handler that only implements `handle_effect` and relies on trait defaults for polling.

**Smallest program (plain words):** A host import adapter that never overrides poll methods; the engine never enters `HostComputationPending` and proceeds as if the host finished computing even when it has not.

## A1102

**Cited sites (read):** `boundary_inputs.rs:1449`, `1606`, `1609` — `prepare_array_view_argument_staged` returns `Ok(None)` when the parameter is not a borrowed byte carrier or when `array_path_type` / `byte_array_length` fail. `2357` — staging skips unhandled leaf kinds with `_ => {}`. `2413-2424` — `overlaps` returns false when `arguments.get(other_index)` is `None`. `2751-2767` — boolean-field presence check `continue`s for case-payload carriers.

**Enclosing functions (read):** `prepare_array_view_argument_staged` (`1430-1620+`); structural initializer `validate` (`2370+`); scalar field `entry::validate` boolean pass (`2742-2782`).

**Caller search (read):** `prepare_array_view_argument_staged` called from `boundary_effects.rs:1330` inside the byte-argument preparation chain (read).

**Upward trace (read):** When `prepare_array_view_argument_staged` returns `None`, `prepare_byte_sequence_argument` continues with other resolvers (`1340-1383+`, read); mismatched shapes tend to error in later arms (e.g. `1347-1351`). `bind_active_structural_arguments` and `validate_boundary_requirements` still run before invocation (`engine_calls.rs:3372-3382`, read). Out-of-range `argument_index` in overlap detection is not treated as overlapping (`2416-2417`); scalar binding uses `machine.structural_parameters.get(argument_index)` and errors with `StructuralScalarFieldArgumentInvalid` when out of range (`2703-2706`, read). Boolean fields on case payloads are intentionally skipped (`2760-2767`, read).

**Input:** Byte-view parameter with wrong carrier shape, or initializer overlap query involving an invalid argument index.

**Verdict basis:** `Ok(None)` is an expected deferral within the preparation chain, not acceptance of an ill-typed argument; downstream steps or parameter lookup reject bad indices.

## A1103

**Cited sites (read):** `boundary_storage_inputs.rs:283-285` — `commit` with `direct_original` calls `record_original_scalar_write` without re-reading the value. `1877-1892` and `2130-2145` — normal-qualification preservation is skipped when `selected.access == StructuralAccess::SharedBorrow`. `1551-1553` — opaque storage leaves skip child capture (`continue` on non-opaque kinds).

**Enclosing functions (read):** `commit` (`273-295`); reference-output preparation (`1877+`, `2130+`); leaf capture loop (`1551+`).

**Caller search (read):** `record_original_scalar_write` — definition `storage_primitives.rs:2368`, calls from `boundary_storage_inputs.rs:285`, `541`, and `boundary_effects.rs:3255` (read).

**Upward trace (read):** `record_original_scalar_write` (`2368-2399`) invalidates provenance or errors on view/representation bytes but does not compare the stored scalar to an expected value for array elements (`2387-2395`, read). Shared-borrow reference outputs bypass the `normal_qualifications_cover` check entirely (`1877`, `2130`). Non-reference opaque leaves do not descend into child storage during capture (`1552-1553`).

**Input:** Host storage commit marked `direct_original` after the host already mutated bytes, or a shared-borrow reference result sourced from a value whose normal qualifications changed.

**Smallest program (plain words):** A mutable host write recorded only as an original write with no read-back check, or a shared-borrow boundary reference result taken from storage that no longer carries the normal qualifications captured at the call site.

## A1104

**Cited sites (read):** `boundary_values.rs:172-180` — `require_scalar_entry_checks_complete` no-ops when `scalar_entry_checks` is absent; otherwise calls `require_complete()`. `1102-1114` — `measure_result_storage` iterates a fixed list of custody row tables. `1402-1404` — `ScalarAggregatePlan::new` skips pushing a type node when `nodes` already contains the same `declaration.id`.

**Enclosing functions (read):** `require_scalar_entry_checks_complete` (`172-180`); `measure_result_storage` (`1097-1124+`); `ScalarAggregatePlan::new` (`1394-1418+`).

**Caller search (read):** `require_scalar_entry_checks_complete` from `engine_terminators.rs:1300` (read). `measure_result_storage` used from result-custody accounting in the same module (read snippets `1097+`).

**Upward trace (read):** Entry terminators require scalar entry checks only when the observation bundle installed a check table (`172-179`); absent checks are defined as complete. Result storage measurement charges only the enumerated maps (`1102-1114`); tables not listed are not measured here (by design per digest note, read). Aggregate planning deduplicates by type id only (`1402-1404`), relying on earlier malformed paths when carriers conflict (`1406-1409`, read).

**Input:** Machine entry with no scalar entry-check table, or aggregate type graph revisiting the same structural type id.

**Verdict basis:** These branches encode intentional “nothing to do” or fixed-coverage policies rather than silent acceptance of invalid programs.

## What I did not read

- Checked-type checker proofs that every ill-typed assignment is rejected before trees reach `checked-interpreter` (would bear on A1097 REACHABLE vs CLOSED in the product path only).
- Full bodies of every production `TerminalEffectHandler` override of `poll_host_computation` beyond spot checks (`embedding.rs`, build adapters, host imports).
- Complete `prepare_byte_sequence_argument` failure paths after all preparers return `None` (A1102 deferral tail).
- Corpus programs demonstrating each REACHABLE scenario on admitted Terminal Psi artifacts.

SLICE COMPLETE
