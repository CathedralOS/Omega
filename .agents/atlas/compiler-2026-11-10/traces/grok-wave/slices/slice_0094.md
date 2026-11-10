slice_0094

| entry id | file:line | verdict | one-line reason |
|----------|-----------|---------|-----------------|
| A0720 | UV:11544-11558 | CLOSED | `None` origin makes `scalar_array_qualifications_hold` fail when qualifications are non-empty (`bytes.rs:941-956`). |
| A0721 | UV:11856-12225 | CLOSED | Unconditional `true` arms are operand-free ops or re-checked in `operation_structural_call_contract_matches` / `primitive_locals::validate`. |
| A0722 | UV:12086-12088 | CLOSED | `accepts_operand_types` requires IEEE float on `raw_result` and boolean on `eligible` when saturating (`control_flow.rs:259-270`). |
| A0723 | UV:12437-12442 | REACHABLE | Empty `call_machine_references` makes `.all` true so callee published-service ceiling is not checked for dynamic `CallStructural`. |
| A0724 | UV:12452 | CLOSED | `_ => true` only for operations that never carry service authority; the five service arms cover the service-touching op kinds. |
| A0725 | UV:13639-13727 | REACHABLE | Non-empty owned projection from `MutableBorrow` skips `structural_access_can_supply` and Affine multiplicity omits `is_partial_affine_path`. |
| A0726 | UV:13447-13448 | CLOSED | `Projected`/`Boundary` calls rely on other guards (`case_projection`, `type_matches`, qualification checks) in the same function. |
| A0727 | UV:13951-13955 | CLOSED | Catalog pins one producer per place; `place_kinds_operation_result` (`UV:7140-7151`) ties ops to that producer. |

### A0720

1. Read `scalar_array_establishment_matches` in full (`UV:11494-11580`, read).
2. Read `scalar_array_qualifications_hold` (`bytes.rs:908-957`, read) and `scalar_constant_value` (`bytes.rs:967-981`, read).
3. Search `scalar_array_establishment_matches` in `omega-rust/`: 3 hits (definition plus two call sites in `unit_validation.rs`; read).
4. Call chain: `validate_function` (`UV:9647-9658`) → `validate_values_and_bindings` (`UV:14453-14499`) → `validate_node_operation_contracts` (`UV:10423-10499`) → `operation_structural_call_contract_matches` (`UV:12550-12551`) → `scalar_array_establishment_matches` (read through `validate_psi_optimization_unit_with_control_cycles` at `UV:18260-18269`, not the full unit entry).
5. At `UV:11526-11559`, the element closure returns `None` when the value is not an `IntegerConstant`, the owning block is the entry block (`UV:11550-11551`), or predecessor bindings are incomplete (`UV:11553-11558`).
6. Input on the accepting-looking path: `EstablishScalarArray` with domain qualifications and at least one element whose origin resolves to `None`.
7. **Verdict: CLOSED** — `None` from the closure makes the element `map` fail (`bytes.rs:944-947`), so `bytes` is `None` and `scalar_array_qualifications_hold` returns false (`bytes.rs:954-956`); only empty qualifications short-circuit to true (`bytes.rs:921-922`).
8. Uncertainty: did not trace producer-stage invariants that might prevent malformed arrays before this validator.

### A0721

1. Read `operation_scalar_types_match` in full (`UV:11811-12388`, read).
2. Search `operation_scalar_types_match` in `omega-rust/`: 3 hits (definition at `UV:11811`, caller `UV:10443`; read).
3. Call chain: `validate_node_operation_contracts` (`UV:10443-10459`) is the sole rejection gate for this function (read); failures become `ScalarOperationContractMismatch` (read).
4. Example accepting branch: match arm at `UV:12055-12072` returns `true` without consulting `definitions` for ops such as `PortWrite`, `PrimitiveScalarRead`, `EstablishScalarArray`.
5. Entry description is partially wrong for cited examples: `PortWrite` carries a literal `u8`, not a `ValueId` (`operations.rs:437-441`, read).
6. **Verdict: CLOSED** — split validation: `EstablishScalarArray` is also checked in `scalar_array_establishment_matches` (`UV:12550-12551`, read); `PrimitiveScalarRead` is checked in `primitive_locals::validate` (`UV:7317-7323`, read); operand-free or embedded-type ops do not need a `definitions` lookup in this function.
7. Uncertainty: did not prove alternate coverage for every variant in the `UV:12055-12072` arm (for example every `AtomicEvent` fence subcase).

### A0722

1. Read `operation_scalar_types_match` `FloatResultPolicy` arm (`UV:12086-12088`, read) and `FloatResultPolicy::accepts_operand_types` in full (`control_flow.rs:259-271`, read).
2. Search `accepts_operand_types` in `omega-rust/`: 7 hits across pipeline and `unit_validation.rs` (count only; read UV and `control_flow.rs` arms).
3. Call chain: same as A0721 through `validate_node_operation_contracts` (`UV:10443-10459`, read).
4. Input: `FloatResultPolicy` with operand value ids missing from `definitions` or with scalar types that do not match the format / boolean requirement.
5. **Verdict: CLOSED** — `policy.accepts_operand_types(*format, scalar)` rejects when `type_of` returns `None` or wrong types (`control_flow.rs:264-269`, read); the digest’s “not read” note is stale relative to the code.
6. Uncertainty: none for this arm.

### A0723

1. Read `operation_service_contract_matches` in full (`UV:12419-12454`, read) and `PsiOptimizationUnit::call_machine_references` (`lib.rs:225-250`, read).
2. Search `operation_service_contract_matches` in `omega-rust/`: 3 hits (read).
3. Call chain: `validate_node_operation_contracts` (`UV:10481-10497`, read); dynamic catalog validation at `UV:1403-1417` allows `None` table rows (read).
4. Input: `CallStructural` with `AbstractCallTarget::Dynamic` where `unit.dynamic_dispatch.tables` yields no machine ids (all `None` rows or no matching table), so the iterator is empty.
5. **Verdict: REACHABLE** — Rust `Iterator::all` on an empty iterator is true (`UV:12437-12441`, read), so `reached_is_published` never runs while `operation_structural_call_contract_matches` can still succeed from interface slot metadata (`UV:12492-12545`, read); a caller could omit callee machines from its published ceiling that a real dispatch target would require.
6. Uncertainty: did not fully trace `validate_function_references` (`UV:15212+`) for whether it always closes this service gap.

### A0724

1. Read `operation_service_contract_matches` in full (`UV:12419-12454`, read).
2. Search: same as A0723 (3 hits).
3. Call chain: `validate_node_operation_contracts` (`UV:10481-10497`, read); unit-level service reach refresh at `UV:15980-16008` (read header only, not full function).
4. Input: any operation kind outside `Call`, `InvokeNoNormal`, `CallStructural`, `BoundaryCall`, `PortWrite` hits `_ => true` (`UV:12452`, read).
5. **Verdict: CLOSED** — those five kinds are the abstract ops that name services or callees with service metadata (`operations.rs:437-447`, read for `PortWrite`/`RegisterAccess`); other ops do not carry service authority, so returning true here is harmless relative to this check’s purpose.
6. Uncertainty: did not read every variant in `AbstractOperation` to exhaustively confirm no other op embeds a `ServiceId`.

### A0725

1. Read `structural_arguments_match_at_invocation` from `UV:13205` through the access/multiplicity logic (`UV:13633-13731`, read); did not read the full function after `UV:13800`.
2. Search `structural_arguments_match` in `omega-rust/`: 9 hits (count; read call sites in `operation_structural_call_contract_matches` only).
3. Call chain: `operation_structural_call_contract_matches` → `structural_arguments_match` (`UV:12537-12545`, read) → `validate_node_operation_contracts` (read).
4. Input: `StructuralProjectionPolicy::Unit` call with formal `Affine` + `Owned`, argument `Owned` with non-empty path, source contract `MutableBorrow` on the same place.
5. **Verdict: REACHABLE** — `UV:13639-13642` exempts `MutableBorrow`→`Owned` with non-empty path from `structural_access_can_supply`; `UV:13722-13727` sets `actual_multiplicity` to `Affine` via `(argument.access == Owned && source.access == MutableBorrow)` without requiring `is_partial_affine_path` (`UV:14235-14240`, read for helper definition).
6. Uncertainty: did not trace `current_ownership::validate_current_ownership_frontier_collecting` (`UV:9660+`) for whether all such moves are rejected later.

### A0726

1. Read `path_shape_matches` and surrounding guards (`UV:13422-13449`, `UV:13633-13645`, read).
2. Same caller chain as A0725 (read).
3. Input: `StructuralProjectionPolicy::Projected` or `Boundary` with arbitrary `argument.path` (including shapes forbidden under `Unit`).
4. **Verdict: CLOSED** — `Projected | Boundary => true` (`UV:13447-13448`, read) is paired with `case_projection` rejection (`UV:13238-13242`, read), `type_matches` / `actual_type` failure (`UV:13609-13624`, read), and qualification loops (`UV:13773-13798`, read).
5. Uncertainty: did not read every `Boundary`-specific helper to end of file.

### A0727

1. Read `structural_source_contract` in full (`UV:13928-13982`, read).
2. Search `structural_source_contract` in `omega-rust/`: many production uses (count via grep list; read `UV:7140-7151`, `UV:13945-13956`, and `validate_structural_root_uniqueness` at `UV:7173-7187` only).
3. Call chain: used from `structural_arguments_match_at_invocation` (`UV:13267-13268`, read) and dozens of other validators (not all read).
4. Input: `PlaceId` with multiple `block_nodes` whose `structural_operation_result_contract` names the same `result.place`; `.find` takes the first in block iteration order (`UV:13947-13955`, read).
5. **Verdict: CLOSED** — `validate_structural_root_uniqueness` (`UV:7173-7187`, read) and `place_kinds_operation_result` (`UV:7140-7151`, read) bind each operation-result place to a single declared `producer` and type; mismatched ops should fail catalog / root-operation checks (`UV:9610-9617`, read call site only).
6. Uncertainty: did not read full `validate_structural_root_operations` body for every op kind.

## What I did not read

- Full bodies of `validate_structural_root_operations`, `validate_function_references`, `current_ownership::validate_current_ownership_frontier_collecting`, and unit catalog validators beyond cited fragments.
- Every `AbstractOperation` variant for service or scalar operand coverage (A0724/A0721).
- Caller chains above `validate_psi_optimization_unit_with_control_cycles` into the CLI/pipeline producers.
- Tests and non-`omega-rust` Omega sources.

SLICE COMPLETE
