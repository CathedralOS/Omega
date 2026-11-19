Second pass for unit 26

Ranges covered (gap_013, 520 lines in all):
- `omega-rust/omega/compilation/representations/abstract-operations/optimization-unit/semantics/src/unit_validation.rs` 17751-18270 (520 lines), read in full.

Short prefix: `SEM/` = `omega-rust/omega/compilation/representations/abstract-operations/optimization-unit/semantics/src/`.

Where the range starts: line 17751 is the closing `);` of the first match arm inside `structural_catalog::type_declarations::validate_structural_fields` (fn starts `SEM/unit_validation.rs:17709`, in `mod type_declarations` at `:17704`, inside `mod structural_catalog`, which the earlier notes place at `:16147`). I read 17700-17750 above the range for the header only. The range then runs through the rest of `type_declarations`, all of `mod witnesses`, the re-export block of `structural_catalog` (closes at `:18203`), and the six top-level entry functions of the file (`:18204-18270`). The earlier notes (`unit_026.md`) listed those entry functions as unread; nothing in my range contradicts them (their line numbers `:18204, 18213, 18229, 18240, 18246, 18260` match).

---

## A. `structural_catalog::type_declarations` tail (17751-17948)

### A.1 `validate_structural_fields` (fn at 17709, range covers 17751-17808)

**Purpose.** Checks one list of field declarations (a record's fields, a mixed type's fields, or one sum case's fields) for canonical order, unique non-absent identities, and a legal pairing of field type with `BindingRelevance` (relevant vs erased).

**Inputs/outputs.** Reads `&[terminal_psi::StructuralFieldDeclaration]` (fields `id`, `identity`, `field_type`, `relevance`), the owning `StructuralTypeId`, an optional case id and a `has_provider_attachment` flag (`:17709-17714`). Returns `Ok(())` or one of `NonCanonicalStructuralFieldOrder`, `InvalidStructuralFieldIdentity`, `InvalidErasedStructuralField` (read; the first two before the range at `:17715-17734`).

**How it decides (all read).** Keyed on declared ids and on the field's declared type/relevance pair, never on names:
- Order: strictly ascending `id` (`:17715`); identity not `ABSENT` and unique inside the list (`:17725-17726`).
- A `match (&field.field_type, field.relevance)` with these arms, in order:
  1. `Atomic | AtomicBoundedInteger` (any relevance) whose `atomic_scalar_type()` is `None` -> `InvalidStructuralFieldIdentity` (not an "erased" error; `:17741-17752`).
  2. `PredicateByteSequence { carrier, predicates }` + `Relevant`: accepted only if `predicates` non-empty and `carrier.supports_field_predicates()` (`:17753-17759`); any other pairing for `PredicateByteSequence` -> `InvalidErasedStructuralField` (`:17760-17762`).
  3. `Erased { type_identity }` with `ABSENT` identity -> error (`:17763-17767`).
  4. `Erased` + `Erased` -> accepted (`:17768-17771`); `Erased` + `Relevant` -> accepted only when `has_provider_attachment` (`:17772-17775`), otherwise error (`:17776-17779`).
  5. `Scalar | Atomic | AtomicBoundedInteger | RefinedScalar | BoundedInteger | IeeeFloat | Structural` + `Erased` -> error (`:17780-17789`).
  6. The same list plus `ByteSequence` + `Relevant`, and `ByteSequence` + `Erased`, -> accepted (`:17790-17804`).
- The match has no `_` arm, so adding a field-type variant without a new arm is a compile error (read: the arms at `:17740-17805` end without a wildcard).

**Cases by shape.** `has_provider_attachment` is the only context flag. Cases call it with `false` (`:17840`), so an erased-but-relevant field is never legal inside a sum case. Record/mixed fields get the flag from the caller (`:16265, 16276`, not read; those lines are in the earlier reader's unread span, so I only grepped them).

**Same job elsewhere.** Suspected: Terminal Psi's own foundation validation checks the same field pairing under different names. I grepped `fn validate_structural_fields` across `omega-rust`: one definition (this file). `InvalidErasedStructuralField|fn structural_children|fn validate_structural_cases` count 1 in `omega-rust/psi/.../terminal-psi/src/types_and_values.rs` and 3 in `SEM/unit_validation.rs`, 1 in `SEM/error.rs`. I did not read the Terminal Psi side, so the duplicate is not established.

**Unused or one-caller.** `grep -n "validate_structural_fields\b" omega-rust --glob *.rs`: 5 lines = 1 import (`:16157`), 2 calls in `structural_catalog::catalog` (`:16265, 16276`), 1 definition (`:17709`), 1 call from `validate_structural_cases` (`:17840`). So 3 call sites.

**Fail-open.** None seen. Every arm that accepts is an explicit pairing; the unlisted pairings (e.g. `Structural` + `Erased`) are errors. One oddity, not a risk: arm 1 reports a missing atomic scalar type as an identity error (`:17745-17751`).

### A.2 `validate_structural_cases` (17810-17843)

**Purpose.** Checks the case list of a sum or mixed type: non-empty, ascending case ids, unique non-absent identities, then every case's fields.

**How it decides.** Empty list -> `EmptyStructuralSum` (`:17814-17818`); `windows(2)` with `>=` on `id` -> `NonCanonicalStructuralCaseOrder` (`:17819-17825`); identity `ABSENT` or repeated -> `InvalidStructuralCaseIdentity` (`:17826-17837`); then `validate_structural_fields(.., Some(case.id), &case.fields, false)` per case (`:17839-17841`). Keyed on ids only.

**Unused or one-caller.** `grep -n "validate_structural_cases\b"`: 2 call sites (`:16273, 16277`) plus import `:16157` and definition `:17810`.

**Fail-open.** None seen.

### A.3 `validate_structural_type_graph` (17845-17906)

**Purpose.** Rejects structural types that contain themselves by value (a by-value cycle). Depth-first walk with an `active` set (current path) and a `complete` set (done).

**Shape.** Takes `&BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>` (`:17846`); inner `visit` (`:17848`).

**How it decides.** `active.insert(id)` returning false -> `RecursiveStructuralType(id)` (`:17857-17859`). It follows `field_type.as_structural()` of record fields, all cases' fields of sums, and both for mixed (`:17867-17893`), and `FixedArray.element` (`:17874`). `Reference`, `ElementView`, `PrimitiveScalar`, `AtomicCell`, `ByteSequence` are leaves (`:17862-17866`), so a type that reaches itself only through a `Reference` or `ElementView` is accepted (read; by design, since those are not by-value).

**Panics, not accepts.** `types[&id]` at `:17860` panics if a field names a structural type that is not in the map. The caller (`:16350`, not read) must have checked field targets earlier; I did not trace that. Recursion depth is the type nesting depth (read; no explicit limit).

**Same job elsewhere (read).** Near copy at `omega-rust/psi/compilation/representations/terminal-psi/codec/src/foundation_validation.rs:11741-11803`: same `active`/`complete` sets, same shape list (`:11758-11791`), but it takes a `&[StructuralTypeDeclaration]`, finds the type with `find_structural_type(..).expect(..)`, and returns `CodecError` (`:11754`). Two copies of one walk in two crates.

**Unused or one-caller.** `grep -n "validate_structural_type_graph\b"`: 1 call in `SEM/unit_validation.rs` (`:16350`); the other 4 matches are the Terminal Psi copy (`foundation_validation.rs:9203, 9249, 10367, 11741`).

**Fail-open.** None seen.

### A.4 `attachment_storage_types` (17908-17947, `pub(super)`)

**Purpose.** Given the structural types that provider attachments name, returns the set of all types reachable by value from them (the "storage" types), by a worklist over children.

**How it decides.** Worklist `pending`; a type is added once (`:17916`); a type id that is not in `types` is added to the set but not expanded (`:17919-17921`, `continue`). Children come from `terminal_psi::structural_children(fields)` for records, mixed, sums, and `FixedArray.element` (`:17925-17938`); leaf shapes add nothing (`:17939-17943`).

**Same job elsewhere (read).** Line-for-line the same function at `omega-rust/psi/.../terminal-psi/codec/src/foundation_validation.rs:8566-8605` (`pub(crate)`). I compared the two bodies by reading both: same signature, same `match`, same `retain_fields` closure; the only difference is that mine imports `structural_children` in the `use` line (`:17912`) and the Terminal Psi copy has it in scope from elsewhere (`:8570-8581`).

**Unused or one-caller.** `grep -n "attachment_storage_types\b"`: this copy has 1 call (`:16215`) plus import (`:16157`); the Terminal Psi copy has 1 call (`foundation_validation.rs:10355`).

**Fail-open.** An id missing from `types` is silently kept in the storage set (`:17919-17921`) with no error. Which input reaches it: an attachment naming an unknown structural type. Whether that was rejected earlier by the caller at `:16215` was not traced: **open**. Effect of keeping it: it counts as "storage" in the set that `has_provider_attachment` and erased-field decisions use (inferred from the name and the call at `:16215`; call site not read).

---

## B. `structural_catalog::witnesses` (17949-18171)

Imports `expected_provenance` from `derived_metadata` (`:17950`); `AbstractOperation as O` (`:17952`).

### B.1 `validate_byte_sequence_literal_witnesses` (17958-17998)

**Purpose.** Checks that the function's `EstablishByteSequenceLiteral` operations are exactly the set of literal places the function's structural catalog says it needs: no missing, no extra, no changed place or type.

**Inputs/outputs.** `function: &PsiOptimizationFunction`, `expected_literals: &[(StructuralPlaceDeclaration, StructuralTypeDeclaration)]` (`:17958-17961`). Returns `ByteSequenceLiteralEstablishmentMismatch(machine)` on any difference.

**How it decides.** Builds a map from `place.id` to `(place, type)` (`:17962-17965`). For each block node of kind `EstablishByteSequenceLiteral` it counts `actual` and removes the expected entry; mismatch if the id is absent or if the whole `place` or whole `structural_type` differs (`:17977-17988`, `is_none_or`). At the end requires `actual == expected_literals.len()` and the map empty (`:17990`). Keyed on `PlaceId` plus whole-declaration equality.

**Unused or one-caller.** `grep -n` across `omega-rust`: 1 call site (`:9603` inside `validate_function`, `.rs` file unread by me) plus re-export `:18187` and import `:2746`.

**Fail-open.** None seen. Duplicate literal ops on one place are caught (second `remove` returns `None`, `:17977-17979`).

### B.2 `validate_trivial_affine_local_witnesses` (18000-18170)

**Purpose.** Checks how "trivial affine locals" (locals that need no cleanup call) are carried. They are either spelled out as separate `EstablishTrivialAffineLocal` operations ("explicit") or compressed into the `trivial_affine_locals` list of the single `ReturnStructural` ("compressed"). The function accepts exactly those two forms and checks them against `expected_locals` from the function's catalog.

**Inputs/outputs.** `function`, `expected_locals: &[(StructuralPlaceDeclaration, StructuralTypeDeclaration)]` (`:18000-18003`). Errors: `TrivialAffineLocalEstablishmentMismatch(machine)`, `StructuralReturnTrivialAffineLocalsMismatch(site)`, `StructuralReturnTrivialAffineShapeMismatch(site)`, `StructuralReturnAffineDiscardsMismatch(site)`.

**How it decides.**
1. Collects explicit `EstablishTrivialAffineLocal { psi_operation, place, structural_type }` nodes and every `ReturnStructural`'s `(trivial_affine_locals, cleanup_actions)` (`:18004-18025`).
2. Explicit form (any explicit node exists): every return's compressed list must be empty, count must equal `expected_locals.len()`, and each node's place and type must equal the expected pair in order (`:18027-18043`); then `return Ok(())` (`:18044`). So in this form the return's `cleanup_actions` and the function's parameter shape are not checked here (read; the early return at `:18044`).
3. Compressed form: if `expected_locals` is non-empty there must be exactly one `ReturnStructural` (`:18047-18053`).
4. Builds `executable_operations`: the set of `PsiProvenance::Operation` ids from `expected_provenance` of every non-`ReturnStructural` node (`:18055-18063`).
5. For each `ReturnStructural` (`:18065-18168`): if both its list and `expected_locals` are empty, `continue` (`:18076-18078`). Otherwise the list must match `expected_locals` by length, place and type, each hidden `psi_operation` id must be unique and must not appear in `executable_operations` (`:18081-18099`). Then the shape: `structural_parameters` must have a first parameter and the result must be structural (`:18101-18114`); no scalar `parameters`; the first parameter's place equals the return `source`; first parameter and result both `Linear`, with equal type, qualifications and projected qualifications, and different places; all other structural parameters `Affine` with no qualifications (`:18115-18141`). Finally `cleanup_actions` must equal `DiscardRoot` of each hidden local in reverse order followed by `DiscardRoot` of each non-first structural parameter in reverse order (`:18142-18166`).

**Cases by shape.** The compressed form is only legal for a function with exactly one scalar-free, linear-in/linear-out structural signature whose extra structural parameters are all affine and unqualified (`:18115-18134`). There is no comment at the site naming this as a limit; it is the closed list of accepted shapes. Any other function with hidden trivial locals fails with `StructuralReturnTrivialAffineShapeMismatch`.

**Panics.** `u32::try_from(node_index).expect("unit node index fits u32")` at `:18079`.

**Same job elsewhere.** `expected_provenance` use mirrors `derived_metadata::provenance::expected_provenance` (`:2602`, earlier notes) which also lists the hidden locals as `Operation` provenance for `ReturnStructural`. Also `validate_byte_sequence_literal_witnesses` (above) is the same "expected set vs actual operations" pattern (`:17958` vs `:18000`).

**Unused or one-caller.** `grep -n`: 1 call site (`:9604`), plus re-export `:18187`, import `:2747`.

**Fail-open.**
- `:18044`: explicit form returns `Ok` without examining the returns' `cleanup_actions` or parameter shape. A function with explicit trivial locals but a wrong `ReturnStructural.cleanup_actions` passes this function; whether the cleanup actions are validated elsewhere (for example `affine_authority` at `SEM/unit_validation.rs:574`, or edge validation) was not traced in my range: **open**.
- `:18076-18078`: a `ReturnStructural` with empty list while `expected_locals` is empty is skipped for the cleanup-discard check (`:18155-18166`), so only returns that carry hidden locals have `cleanup_actions` checked here. No comment at the site: **open** (inferred: with no hidden locals there is nothing for this function to check).
- `:18047`: if `expected_locals` is empty and no explicit nodes exist, nothing about returns is required here (it falls through to the loop, which skips every return). Wrong program that gets through: a return carrying a non-empty list while expected is empty is NOT accepted (length mismatch at `:18081`, traced by reading), so this is closed in that direction.

---

## C. Re-exports and `index_structural_catalogs` (18172-18203)

- Re-exports from `paths`, `storage_projection`, `function_catalog`, `provider_specialization`, `witnesses` (`:18172-18188`). `pub use storage_projection::{StructuralStorageProjection, structural_borrowed_storage_projection, structural_storage_projection}` (`:18174-18177`) are the public ones re-exported again in `lib.rs` (earlier notes). I did not grep outside use counts for the `paths` and `storage_projection` re-exports.
- `index_structural_catalogs(unit)` (`:18190-18202`, `pub(crate)`): calls `catalog::index_structural_types(unit)` then `catalog::index_structural_domains(unit, &types)` and returns both `BTreeMap`s (`:18199-18201`). Both callees are in `mod catalog`, which I did not read.
- Callers (grep): `SEM/unit_validation.rs:1189` (`catalogs::index_and_validate_unit_catalogs`) and `:18243` (`validate_psi_structural_catalog`). 2 call sites.

---

## D. Top-level entry functions (18204-18270)

All take a `PsiOptimizationUnit`; none mutates it except `revalidate_rewrite_output`.

- `validate_psi_optimization_unit(unit)` (`:18204`, `pub(crate)`): calls `..._with_control_cycles` with `ControlCyclePolicy::default()` (`:18207-18210`). Call sites outside its definition (grep `validate_psi_optimization_unit\(`): `candidates.rs:422`, `candidates.rs:1802`, `current_value_ranges.rs:482`, and `unit_validation.rs:18225` (inside `revalidate_rewrite_output`): 4.
- `revalidate_rewrite_output(output, machine)` (`:18213`, `pub(crate)`): takes `&mut` unit, then (a) replaces `output_function.facts` with `function_structure::reconstruct_fact_index(..)` (`:18220`), (b) replaces `declared_places` with `derived_metadata::reconstruct_declared_places(..)?` (`:18221-18222`), (c) `services::refresh_root_service_reach(output)?` (`:18223`), (d) recomputes `output.identity` (`:18224`), (e) runs `validate_psi_optimization_unit(output)` (`:18225`). So for a rewrite candidate the fact index, declared places, root service reach and content identity are overwritten from the operations before validation, not compared with what the producer wrote. Judgment is keyed on `MachineId` (the one function that is re-derived). `.expect("output function exists")` panics if the machine is absent (`:18219, 18226`). Call sites: `candidates.rs:334, 2463, 10680` (3).
- `validate_psi_optimization_unit_with_admitted_cycle_machines(unit, &[MachineId])` (`:18229`, `pub`): builds a `ControlCyclePolicy`, calls `admit(machine)` for each (`:18233-18236`), then the common function. Outside callers: `pipeline/00_terminal-psi-to-abstract-operations/src/artifact_admission.rs:1126`, `pipeline/01_abstract-operations-to-abstract-operations/src/pass_manager.rs:1313` (2). Documented in `lib.rs:52`.
- `validate_psi_structural_catalog(unit)` (`:18240`, `pub`): `index_structural_catalogs(unit).map(|_| ())` (`:18243`), so it runs only the structural type/domain catalog indexing and validation. Call sites: `compiler/tests/support/optimization_catalog_controls.rs:29, 64, 93` (3, all test support). Documented in `lib.rs:53`.
- `reconstruct_ownership_frontier_facts_with_admitted_cycle_machines(unit, &[MachineId])` (`:18246`, `pub`): runs `identity_indexes::validate_identity_and_fact_indexes(unit)?`, builds the same policy, runs `catalogs::index_and_validate_unit_catalogs(unit, &policy)?` and takes `.ownership_frontiers` from the result, sorted by `(machine, site)` (`:18250-18256`). Returns the facts as a `Vec` and does not itself compare them with `unit.ownership_frontier_facts`. Outside callers: `artifact_admission.rs:1122`, `pass_manager.rs:1307` (2). Documented in `lib.rs:54`.
- `validate_psi_optimization_unit_with_control_cycles(unit, policy)` (`:18260`, `pub(crate)`): the fixed phase order (read): `identity_indexes::validate_identity_and_fact_indexes` (`:18264`) -> `catalogs::index_and_validate_unit_catalogs` (`:18265`) -> `no_normal_calls::validate(unit, &indexes.machines)` (`:18266`) -> `runtime_evidence::validate` (`:18267`) -> `affine_authority::validate_retained_ownership_authority` (`:18268`) -> `catalogs::validate_final_authorities` (`:18269`). Callers: `:18207` and `:18237` (2, both in this file). This order matches the module-header comment summarised in the earlier notes (`unit_026.md` section 4), apart from the per-function CFG and per-operation phases, which run inside `index_and_validate_unit_catalogs` (inferred from the earlier notes at `catalogs` `:1233`; not read by me).

**Fail-open.** None seen in the entry functions: every phase call uses `?`. Two properties for a reader: `revalidate_rewrite_output` trusts the reconstructed values it wrote itself (above); and the retained-ownership phase at `:18268` is skipped inside when a unit has no `ownership_frontier_facts` (earlier notes `SEM/unit_validation.rs:883`), so for such a unit the whole retained-ownership check is a no-op (stated in the earlier notes as read; I did not re-read `:883`).

---

## Stated limits in my range
None. The range has no comments at all (checked by reading every line; no `//` lines in 17751-18270).

## Coverage
- Read in full: `unit_validation.rs` 17751-18270 = 520 lines.
- Skimmed: none.
- Not read: none. (Context lines 17700-17750 were read for the header only; they do not count.)

Counts came from the Grep tool (content mode, `*.rs` under `omega-rust`), patterns as quoted above; counts are call-site lines I counted from the full uncapped output.

## Five things a maintainer most needs to know about this range
1. `validate_structural_fields` (`:17709`) has an exhaustive field-type by relevance `match` with no wildcard; the only context switch is `has_provider_attachment`, and sum cases always pass `false` (`:17840`).
2. `attachment_storage_types` (`:17908`) is an exact copy of `foundation_validation.rs:8566`, and `validate_structural_type_graph` (`:17845`) is a near copy of `foundation_validation.rs:11741`; each has one caller in each crate.
3. `validate_trivial_affine_local_witnesses` accepts two forms (explicit ops or compressed into the one `ReturnStructural`); in the explicit form it returns `Ok` at `:18044` without looking at the returns' cleanup actions; in the compressed form only returns that carry hidden locals have their discards checked (`:18076`).
4. `revalidate_rewrite_output` (`:18213`) overwrites `facts`, `declared_places`, root service reach and `identity` before validating, so a candidate's own values for those are never compared.
5. The six entry functions are the whole public surface of this file's validation; the phase order is fixed at `:18264-18269` and `validate_psi_optimization_unit` has 4 call sites (`candidates.rs` x2, `current_value_ranges.rs`, and `revalidate_rewrite_output`).

## What I am unsure about
- Whether `validate_structural_type_graph`'s `types[&id]` panic (`:17860`) is prevented by earlier checks in `catalog` (`:16350` caller not read).
- Whether the missing-id case in `attachment_storage_types` (`:17919-17921`) is rejected before the call at `:16215`.
- Whether the early `Ok` at `:18044` and the skip at `:18076` are covered by a cleanup-action check in another phase.
- I did not grep outside call counts for the `paths` / `storage_projection` re-exports at `:18172-18188`.
