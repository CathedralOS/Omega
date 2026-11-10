# slice_0133

Preamble (read): `digests/area_12.md:1-12` — `codec/` = `omega-rust/psi/compilation/representations/terminal-psi/codec/src/`; structural foundation runs in `decode_module` via `validate_structural_foundation` (`digests/area_12.md:26-27`, `semantic_module.rs:4109` read).

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A1021 | `foundation_validation.rs:11215-11219` | REACHABLE | Default arm only insists the operation result is scalar; many kinds (e.g. `PrimitiveScalarRead`, arithmetic constants) have no dedicated foundation checks in this match. |
| A1022 | `foundation_validation.rs:2331-2337`, `:29-58` | CLOSED | Digest correction stands: every static module invocation is re-validated via `validate_module_invocation_entry` → `validate_call_tail` → `validate_structural_arguments` / `validate_claim_actions`. |
| A1023 | `foundation_validation.rs:226-229`, `:292-368` | REACHABLE | When no dispatch table row exists for the interface/slot, the per-callee `validate_structural_arguments` loop never runs; only slot arity and claim-action checks remain. |
| A1024 | `foundation_validation.rs:10483-10502` | REACHABLE | `validate_boundary_application_operations` only requires each demand’s operation id to appear somewhere in `module.operations()`, not that that operation is a `BoundaryCall`. |
| A1025 | `foundation_validation.rs:4613`, `:3027-3029`, `:3190-3192`, `:9401-9415` | REACHABLE | Several unchecked corners in proof-scope term walk, zero declaration ids, and task-activation crossing/carry metadata. |
| A1026 | `mathematical_proof_actual.rs:776` | REACHABLE | `ProofTerm::Formal` / `InvocationFormal` call `grants` then discard the set; only type-identity alignment is checked—no `check_grants` on that path. |
| A1027 | `sections.rs:115-116` | REACHABLE | Optional installation bytes are hashed into the manifest without decoding or structural validation of their contents. |
| A1028 | `module_wire.rs:3395`, `wire_fields.rs:179-181` | REACHABLE | `reader.count()` is a bare `u32`; `type_parameter_count` and other nearby loops use it without `bounded_count` (`wire_fields.rs:231-245`). |

## A1021

**Verdict:** REACHABLE (read)

**Cited site:** `omega-rust/psi/compilation/representations/terminal-psi/codec/src/foundation_validation.rs:11215-11219` — `_ =>` requires `OperationResult::Scalar(_)` and nothing else.

**Enclosing function (read):** `validate_operation_foundation` at `foundation_validation.rs:10877-11222`. Prelude at `:10883-10921` handles structural results and `ReadScalarApplication` carrier shape only for that kind.

**Callers (search, read):** `\bvalidate_operation_foundation\b` in `omega-rust` → **3** matches in `foundation_validation.rs` (definition `:10877`, re-export/use in `machine_foundations` mod `:2258`, sole production call `machine_foundations::validate_machine` `:2322`). Upstream: `validate_machine` `:2276-2344` → `validate_structural_foundation` `:10256-10258` → `decode_module` `semantic_module.rs:4109` (read).

**Kinds reaching `_ =>` (read):** Compared `OperationKind` variants `control_flow.rs:1042-1571` to the `match` at `foundation_validation.rs:10923-11219`. Kinds with no dedicated arm include at least: `ReadScalarApplication` (only the pre-match carrier check), `StorageAddress`, `PrimitiveScalarRead`, `ByteSequencePredicate`, `ByteSequenceEqual`, `SelectImplementation`, and the scalar constant/arithmetic/compare families (`IntegerConstant`, `BooleanNot`, `ExactIntegerAdd`, `TrappingInteger`, etc.).

**Input (plain words):** A canonical byte module whose machine block contains e.g. `PrimitiveScalarRead` with a `PlaceId` that is not declared on the machine, but whose operation result is a well-typed scalar, reaches `_ =>` at decode foundation validation and is accepted if no other validator rejects the place.

## A1022

**Verdict:** CLOSED (read)

**Cited correction (read):** TSV notes unit 080’s “never checked” claim does not match; `validate_machine` runs `validate_module_invocation_entry` for every `machine.module_invocations()` (`foundation_validation.rs:2331-2337`).

**Trace (read):** `validate_module_invocation_entry` `:61-126` arity-checks, then calls `validate_call_tail` `:119-125`. `validate_call_tail` `:29-58` always calls `validate_structural_arguments` `:36-42` and `validate_claim_actions` `:43-49`.

**Static call kinds (read):** `OperationKind::module_invocation` in `control_flow.rs:387-484` covers `Call`, `CallUnit`, `CallStructuralScalar`, `CallStructuralWithScalarArguments`, and `CallStructural`; `Block::module_invocations` `:524-555` collects block operations and `InvokeNoNormal` terminators.

**Note (read):** `validate_call_unit` `:142-166` and `validate_call_structural_scalar` `:372-399` used from `validate_operation_foundation` check arity only, but the parallel `module_invocations` pass still runs `validate_call_tail` for the same operations, so block-embedded static calls are not exempt.

## A1023

**Verdict:** REACHABLE (read)

**Cited sites (read):** `validate_call_dynamic` `:168-370`. Slot shape arity at `:226-229`; per-row callee checks in `for row in module.dynamic_dispatch.tables…` `:292-361` call `validate_structural_arguments` only when a row exists; `:362-368` always runs `validate_claim_actions`.

**Callers (read):** Invoked only from `validate_operation_foundation` match arm `OperationKind::CallDynamic` at `foundation_validation.rs:11107` → `validate_machine` `:2322`.

**Input (plain words):** A `CallDynamic` operation whose interface/slot is valid in `dynamic_dispatch.interface` but no `dynamic_dispatch.tables` row is populated for that slot passes count checks against the slot shape while structural argument places/access are not validated against any callee machine.

## A1024

**Verdict:** REACHABLE (read)

**Cited site (read):** `validate_boundary_application_operations` `foundation_validation.rs:10483-10502` — scans `module.operations()` for ids listed in `boundary_application_demands`, rejects missing or duplicate ids, never inspects `operation.kind`.

**Callers (search, read):** `\bvalidate_boundary_application_operations\b` in `omega-rust` → **2** matches (`:10483` definition, `:10286` call from `validate_structural_foundation`).

**Related but partial (read):** `scalar_applications::validate` ties demands to `BoundaryCall` when validating proof-scope operators (`:3746-3758`), but that gate does not run for every demand listed in `boundary_application_demands`.

**Input (plain words):** Manifest lists a boundary application demand pointing at an operation id that exists as a non-`BoundaryCall` (e.g. `Call`); foundation validation accepts the module.

## A1025

**Verdict:** REACHABLE (read)

**Sub-claims from TSV, all read in current tree:**

1. **Proposition walk (`:4542-4616`, `_ =>` at `:4613`):** `validate` destructures only selected `Proposition` variants into scalar terms (`Equal`, integer-math comparisons, `StructuralIdentity`, connectives). Variants such as `Truth`, `Falsehood`, `Atom(_)`, `ByteSequenceEqual`, `StructuralCaseMembership`, `ContentConservation`, and `IeeeFloatComparison` fall through `_ =>` without pushing operands into `validate_terms` (`proposition.rs:1806-1847` read for variant set).

2. **Zero declaration ids (`:3026-3029`, `:3188-3192`):** `proof_scope::validate` skips `declaration(*owner)` when `owner.0 == 0`, and skips call-graph identity checks when `identity.0 == 0`.

3. **Task activation (`task_activation::validate` `:9208-9417`):** Validates crossings’ `live_carry` shape (`:9401-9414`) but never reads `demand.carry_obligations` (`terminal_module.rs:973` read); no check relating `crossings` to `frames` identities beyond independent frame/call graphs (`:9324-9399`).

**Input (plain words):** Examples: (a) boundary scalar `requires` carrying a `ContentConservation` proposition with malformed innards; (b) proof scope pending-evaluation owner id `0` referencing no declaration; (c) task activation demand with inconsistent `carry_obligations` flags versus crossing `preserve_*` fields—all can pass the cited validators.

## A1026

**Verdict:** REACHABLE (read)

**Cited site (read):** `mathematical_proof_actual.rs:772-801` — branch for `ProofTerm::Formal` / `InvocationFormal` sets `let _authorized = grants(carrier)?` at `:776` then only compares type identities `:795-800`. Mathematical branches push into `pending` and later run `check_grants` `:818-825`.

**Callers (search, read):** `\bverify_module_mathematical_actuals\b` in `omega-rust` → **7** matches (definition `mathematical_proof_actual.rs:653`, re-export `lib.rs:124`, tests under `omega/compilation/compiler/tests/support/`).

**Input (plain words):** A paired actual/trusted module where a mathematical slot’s actual is a formal proof reference: if the caller-supplied `grants` closure returns `Ok` and type identities align, verification succeeds without assumption-closure checking on that formal actual.

## A1027

**Verdict:** REACHABLE (read)

**Cited sites (read):** `sections.rs:95-104` (`build_artifact_manifest` entry) and `sections.rs:115-116` (`installation_record.map(|bytes| hash_section(INSTALLATION_DOMAIN, bytes))`). Optimization record is validated against semantic identity `:112-114`; installation blob is not decoded.

**Callers (read):** `build_artifact_manifest` referenced from `sections.rs` and `canonical_artifact.rs` (grep count in codec crate: 4 sites, read names only).

**Input (plain words):** Arbitrary installation byte payload hashed into the artifact manifest fingerprint without wire or foundation checks; acceptance depends only on hash inclusion, not installation structure.

## A1028

**Verdict:** REACHABLE (read)

**Cited sites (read):** `module_wire.rs:3395` `let type_parameter_count = reader.count()? as usize;` stored at `:3464` without a loop over that count. `Reader::count` `wire_fields.rs:179-181` delegates to `u32()` with no `remaining()` bound (contrasts `bounded_count` `wire_fields.rs:231-245` used by `decode_counted`).

**Same decode function (read):** `ProofApplicationScope` decode uses raw `for _ in 0..reader.count()?` at `module_wire.rs:3372` and `:3384` (not `bounded_count`; `bounded_count` has **0** uses in `module_wire.rs` per search).

**Input (plain words):** Malformed `PSITERM\0` proof-scope section declaring a huge `count` can drive long/degenerate decode loops before structural foundation runs; `type_parameter_count` itself is not capped at read time beyond fitting in `usize`.

## What I did not read

- Full `foundation_validation.rs` outside cited regions (~11k lines total per digest).
- Entire `module_wire.rs` decode paths beyond proof-scope callable header loop.
- Callers of `build_artifact_manifest` in non-codec crates.
- Whether downstream pipelines always reject samples for the REACHABLE cases above (no runtime/build attempted per instructions).

SLICE COMPLETE
