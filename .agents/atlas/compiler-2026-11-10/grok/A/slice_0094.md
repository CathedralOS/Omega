# slice_0094

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0720 | UV:11544-11558 | CLOSED | `None` origin makes `scalar_array_qualifications_hold` fail when qualifications are non-empty. |
| A0721 | UV:12055-12072 | REACHABLE | `operation_scalar_types_match` returns `true` for many ops; node contracts have no other scalar operand gate. |
| A0722 | UV:12086-12088 | CLOSED | `FloatResultPolicy` arm calls `accepts_operand_types`, which checks result and saturating operand types. |
| A0723 | UV:12437-12442 | REACHABLE | Empty `call_machine_references` makes `.all` vacuous, skipping callee published-ceiling checks. |
| A0724 | UV:12452 | REACHABLE | Wildcard `_ => true` accepts every operation kind outside the five named arms. |
| A0725 | UV:13639-13642 | REACHABLE | Non-empty path bypasses `structural_access_can_supply`; affine formal can use mutable borrow without partial-affine path. |
| A0726 | UV:13447-13448 | REACHABLE | `Projected` and `Boundary` projections set `path_shape_matches` to true for any argument path. |
| A0727 | UV:13951-13955 | UNDECIDED | `.find` picks first producer node; no duplicate-producer rejection traced on this path. |

## A0720

Digest area_05 item 57; preamble UV = `optimization-unit/semantics/src/unit_validation.rs` (read).

Read `scalar_array_establishment_matches` `omega-rust/omega/compilation/representations/abstract-operations/optimization-unit/semantics/src/unit_validation.rs:11494-11580` (read). Element origins are supplied via closure `UV:11525-11560` passed to `terminal_semantics::scalar_array_qualifications_hold` at `UV:11519-11524`.

Origin resolution at `UV:11544-11558` (read): entry-block parameters return `None` at `UV:11550-11551`; non-entry parameters need incoming edge bindings collected at `UV:11553-11558`, else `None`.

Read kernel `scalar_array_qualifications_hold` `omega-rust/psi/compilation/representations/terminal-psi/semantics/src/bytes.rs:908-956` (read). If `qualifications.is_empty()` → `true` at `921-922` without calling `literal`. If non-empty, each element calls `literal(*element)?` at `941-947`; `None` makes `bytes` `None` and the function returns false at `954`.

Search `rg '\bscalar_array_establishment_matches\b' omega-rust` → **3** matches (read), all in `unit_validation.rs`: definition `11494`; import/use in `operation_structural_call_contract_matches` `12551`.

Upstream: `operation_structural_call_contract_matches` is called from `validate_node_operation_contracts` `10461-10479` (read); failure yields `StructuralCallContractMismatch`.

**Input:** `EstablishScalarArray` with non-empty `result.qualifications` and at least one element value whose origin closure returns `None` (e.g. entry parameter at `UV:11550-11551`, or missing incoming bindings at `UV:11558`).

**Decision:** Qualification kernel rejects (`bytes.rs:941-954`); establishment does not pass. `None` does not accept.

## A0721

Digest area_05 item 58 (cites `UV:11856` and others; primary read `UV:12055-12072`).

Read `operation_scalar_types_match` in full `unit_validation.rs:11811-12240+` (read; digest-listed arms at `11856`, `11896`, `11927`, `12055-12072`, `12085`, `12106`, `12225` confirmed). Large wildcard groups return `true` without consulting `definitions` (e.g. `O::PortWrite`, `O::PrimitiveScalarRead`, `O::EstablishScalarArray` at `12055-12072`).

Search `rg '\boperation_scalar_types_match\b' omega-rust` → **3** matches (read): definition `11811`; `use` + call `10411`, `10443` in `validate_node_operation_contracts`.

Read `validate_node_operation_contracts` `10423-10499` (read): on false, `ScalarOperationContractMismatch` at `10451-10458`; no further scalar-operand typing in this function before structural-call and service checks.

`PrimitiveScalarRead` has richer checks under `validate_structural_root_operations` `7066-7094` (read), but that is a separate path from `operation_scalar_types_match`.

**Input:** A unit node whose operation is e.g. `PortWrite` where port payload value ids’ `definitions` scalar types are wrong or missing, while the unit still lists consistent-enough definitions for other checks.

**Decision:** `operation_scalar_types_match` returns `true` at `12069`; node contract validation accepts unless another validator rejects (not traced for `PortWrite` operands in this read).

## A0722

Digest area_05 item 59 (`not read` at digest time).

Read `operation_scalar_types_match` arm `unit_validation.rs:12085-12088` (read): `policy.accepts_operand_types(*format, scalar)` where `scalar` closes over `definitions` at `11822`.

Read `FloatResultPolicy::accepts_operand_types` `omega-rust/psi/compilation/representations/terminal-psi/src/control_flow.rs:259-271` (read): requires `raw_result` type `IeeeFloat(format)`; for `Saturating`, also requires `eligible` operand `Boolean`.

Search `rg '\baccepts_operand_types\b' omega-rust` → **6** matches (read): definition `control_flow.rs:259`; UV use `12087`; pipeline lowering/selection sites (not opened).

**Input:** `O::FloatResultPolicy` on a node visited by `validate_node_operation_contracts`.

**Decision:** Operand types are checked via `accepts_operand_types`; mismatch returns false from `operation_scalar_types_match`.

## A0723

Digest area_05 item 60.

Read `operation_service_contract_matches` `unit_validation.rs:12419-12454` (read). `CallStructural` arm `12436-12441`: `unit.call_machine_references(operation).all(|machine| … reached_is_published …)`.

Read `PsiOptimizationUnit::call_machine_references` `omega-rust/omega/compilation/representations/abstract-operations/optimization-unit/src/lib.rs:225-250` (read): static machine optional; dynamic `(interface, slot)` chains table rows; non-matching ops yield empty iterator.

Rust: `.all` on empty iterator is `true` (inferred).

Search `rg '\boperation_service_contract_matches\b' omega-rust` → **3** matches (read): definition `12419`; `validate_node_operation_contracts` `10481`; service-reach refresh `15975` region (not fully read).

**Input:** `CallStructural` with dynamic callee whose `call_machine_references` iterator is empty (e.g. no resolved machine in unit tables at `lib.rs:240-249`).

**Decision:** Published-ceiling check does not run; `operation_service_contract_matches` returns `true` for that operation.

## A0724

Digest area_05 item 61.

Read `operation_service_contract_matches` `unit_validation.rs:12432-12453` (read): only `Call`, `InvokeNoNormal`, `CallStructural`, `BoundaryCall`, `PortWrite` arms inspect services; `_ => true` at `12452`.

Caller search: same **3** matches as A0723 (read).

Upstream `validate_node_operation_contracts` `10481-10497` (read): false → `OperationServiceContractMismatch`.

**Input:** Any other `AbstractOperation` variant on a validated node (e.g. `EstablishRecord`, `AtomicEvent`) regardless of service reach semantics.

**Decision:** Service contract helper returns `true` without examination.

## A0725

Digest area_05 item 62; related `UV:13722-13727`.

Read `structural_arguments_match_at_invocation` `unit_validation.rs:13205-13824` (read). Failure guard `13633-13644`: rejects unless `structural_access_can_supply` **or** exception `13639-13642` (mutable-borrow source, owned argument, non-empty path).

Read `structural_access_can_supply` `14204-14218` (read): `MutableBorrow` does not supply `Owned`.

Read multiplicity selection `13722-13727` (read): `Affine` allowed when `parameter.multiplicity == Affine` and `argument.access == Owned` with `source.access == MutableBorrow` **without** `is_partial_affine_path` in that disjunct (contrast `13723-13724`).

Search `rg '\bstructural_arguments_match_at_invocation\b' omega-rust` → **3** matches (read), all `unit_validation.rs`.

Digest wrong program (plain words): structural call passing an owned projection of an affine field out of a place still under mutable borrow, with non-empty path, formal affine owned parameter.

**Input:** `StructuralProjectionPolicy::Unit` call with non-empty path, `Owned` argument, `MutableBorrow` source, affine formal multiplicity; exception at `13639-13642` bypasses access supply check; `13725-13726` can keep affine multiplicity.

**Decision:** Match function can return `true` without `is_partial_affine_path` on that branch.

## A0726

Digest area_05 items 63 / digest § `UV:13447-13448`.

Read `path_shape_matches` inside `structural_arguments_match_at_invocation` `unit_validation.rs:13422-13448` (read). `StructuralProjectionPolicy::Unit` enforces shapes `13423-13445`; `Projected | Boundary => true` at `13447-13448`.

Search `rg '\bstructural_arguments_match_at_invocation\b' omega-rust` → **3** matches (read).

**Input:** Structural call validated with `StructuralProjectionPolicy::Projected` or `Boundary` and an argument path that would fail unit policy shapes (e.g. mixed segment kinds not listed for unit policy).

**Decision:** `path_shape_matches` is true regardless of path content; later checks may still fail, but path shape is not gated here.

## A0727

Digest area_05 item 64.

Read `structural_source_contract` `unit_validation.rs:13928-13982` (read). After arrival parameter, operation results: `block_nodes().filter_map(structural_operation_result_contract).find(|source| result.place == place)` at `13946-13955` — first matching node in iteration order.

Read `validate_structural_place_availability` `6681-6738` (read): `producers.insert` overwrites prior producer for same `PlaceId` at `6705-6714`; no duplicate error in this function.

Read `validate_structural_root_uniqueness` `7173-7187` (read): rejects duplicate `structural_root_key` in `structural_places`, not duplicate operation-result sites.

Read `place_kinds_operation_result` `7140-7151` (read): ties `PlaceId` to one recorded `producer` in `structural_places` table.

Search `rg '\bstructural_source_contract\b' omega-rust` → **87** matches (read): many production uses in `unit_validation.rs`, `current_ownership.rs`, `descriptor_results.rs`, `lib.rs` (count from ripgrep).

**Not determined:** Whether a unit with two distinct operations both declaring `structural_result()` for the same `place` is rejected before this `.find` runs, or whether `.find` can disagree with `structural_places`’ recorded producer.

**Would decide:** Full trace of place/result uniqueness validation on operation nodes (beyond `DuplicateStructuralPlaceRoot` at `7173-7187`).

## What I did not read

- `operation_scalar_types_match` arms after `12225` not cited in the slice row (partial function read).
- `refresh_root_service_reach` body around `UV:15975` beyond the `call_machine_references` extend at `15971`.
- Pipeline lowering uses of `accepts_operand_types` (`lowering_scalars.rs`, legalization/selection) except definition.
- `git show 457adf4a11f` for line drift (current checkout lines matched cited symbols).

SLICE COMPLETE
