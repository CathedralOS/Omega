# Record: unit_062 (area 03-source-stage-05)

Root for all paths: `source-rust/subsystems/compiler/source-to-psi/pipeline/05_prepared-typed-trees-to-lowered-program/`, written `<05>/`.
Files under `<05>/src/read/` are written `read/...`. Every file below was read in full. "read" = I read the lines; "grepped" = a Grep run over `source-rust/`; "inferred" = not read at the other site.
`Program` is the shared reader state (`read.rs`, not in this unit). `Checked<T>` is `Result<T, Refusal>`. `refuse(...)` makes an `Unsupported` refusal ("not supported yet"); `reject(...)` makes a `Violation` ("the spec says this program is wrong").

---

## 1. read/record_parameters.rs (1163 lines; code 1-262, tests 264-1163)

1. **Purpose.** Prepares the "static record" binders of a generic machine: a const parameter whose type is a trait (schema), for example `<const A: Ops>`. It records which requirements the record offers and gives each (binder, requirement, arguments) its own machine identity, separate from the public header of the requirement.
2. **Shape.** `Program::prepare_record_parameter` (:23, classify and form the schema once, returns `Checked<bool>`; false means "ordinary scalar value"), `record_requirement_signature` (:127, finds the member position and prepared signature), `record_requirement` (:176, mints or reuses a `MachineId` per (binder, position, arguments)), `selected_record_operand` (:222, rejoins the actual operand for a selected call), `record_parameter` (:17). Rows are `SourceRecordBinding` / `StaticRecordMember` / `StaticRecordRequirementInstance` (`static_contracts.rs`).
3. **Inputs/outputs.** Reads `typed.static_record_parameter_schema`, `trait_machine_signatures`, `applied_callable_signature`, `declaration_type_parameters`. Mutates `self.static_contracts` (via `insert_record_binding`), and pushes parallel rows on `symbols/signatures/names/value_parameters/value_arguments/scopes/source_scopes/const_scopes/requirements` (:198-206).
4. **How it decides.** Keyed on symbol identity: `declared == parameter` struct equality (:31-45), walking `parent` links to the binder owner (:47-56), `member.requirement == origin` (:140-149), `instance.arguments == arguments` (:190). Compares `signature.symbol != target` (:167). No spelling compare.
5. **Cases by shape.** Refuses a parameter whose `bounds` is not default (:65, "needs its complete declared storage and multiplicity obligations"). Refuses any schema with a `Field` child (:86-89, "ordinary or erased value/evidence field needs its own complete retained account"). Refuses competing applications of one inherited requirement (:93-95).
6. **Same job elsewhere.** `record_requirement` (:176) pushes the same parallel rows that the ordinary `requirement` path pushes (test at :803 calls `program.requirement`, defined outside this unit; suspected duplicate row push, not read).
7. **Unused/one-caller.** grepped `\.record_requirement\(`: 1 non-test call, `read/calls.rs:2486`. `prepare_record_parameter`: 1 call, `read/machine.rs:1791`. `record_requirement_signature`: callers include `read/transitions.rs:540` (read) and tests.
8. **Stated limits.** "needs its own complete retained account, never omission" (:84-85, comment says fields are not omitted); "Schema formation is cached; lexical visibility remains caller-specific" (:46).
9. **Fail-open.** `prepare_record_parameter` returns `Ok(false)` when `static_record_parameter_schema` yields `None` (:59-64), leaving the carrier to the scalar reader. Traced: only a parameter with no record schema reaches it; stated in the doc at :21-22. No other accepting path seen.

Tests (264-1163, read): fixtures with two binders, checks that distinct binders get distinct instance ids, header never aliases a record projection, finite reach does not close omitted reach, crossed suppliers refused. Test-only; not summarised further.

---

## 2. read/references.rs (956 lines; code 1-571, tests 573-956)

1. **Purpose.** Reads a reference type (`@'a T`, `@mutable T`) into a `ValueType::Record` row of shape `StorageShape::Reference`, and forms the loan regions of a borrow. It also decides whether a place reached through a reference grants the access the use needs.
2. **Shape.** `reference_type` (:15), `reference_referent` (:157), `borrow_value` (:195), `finish_reference_value` (:274), `loan_regions` (:311), `formed_loan_regions` (:336), `projected_loan` (:356), `storage_loan_regions` (:420), `reference_place_access` (:483), free fns `reference_access_violation` (:545) and `access_named` (:564).
3. **Inputs/outputs.** Reads `typed.type_reference_table`, `source_scope`, `region` machinery (`regions.rs`), `storage_path::walk`. Mutates `self.records` and `self.record_sources` (pushes a `RecordType` with `StorageShape::Reference`, :129-153) and region tuples.
4. **How it decides.** Reference reuse is keyed on the storage identity string: `record.identity == identity` and `constant_input_owner` (:112-119). Access compares are on `StructuralAccess` values (:509 `admits_referent`). Slot lookups key on slot index (:454).
5. **Cases by shape.** `storage_loan_regions` accepts only a root of kind `Slot` or `OwnedTemporary` else refuses "reference formation without an exact storage root" (:452-470). `projected_loan` handles `Referent`, `Index`, `ByteElement` only, returns `None` for any other projection kind (:366-369). A slice reference returns `None` from `reference_type` so it is handled elsewhere (:42-51).
6. **Same job elsewhere.** The access-from-`language_core::ReferenceAccess` to `StructuralAccess` three-way mapping appears at :52-56, :234-245, :290-299 (in `result_lifetimes.rs`), :410-411, :491-494 (inferred identical, read). `reference_access` in `reference_results.rs:1144` repeats the same mapping in reverse (read).
7. **Unused/one-caller.** Not individually grepped (all `pub(super)`).
8. **Stated limits.** none seen beyond refusals.
9. **Fail-open.** `reference_place_access` returns `Ok(false)` (not crossed) for a place reached through no reference (:540); "traced": the caller then falls back to the binding's `assignable` check (`statements.rs:281`). `projected_loan` `(false, _) => Ok(None)` (:414) for a projection that is not a descriptor crossing; open. Nothing returns `true` by default.

Tests (573-956, read): reference rows use the substituted element type; two references with the same spelling `a` but different owners stay distinct; `same_signature` alpha-renames lifetimes.

---

## 3. read/reflection.rs (630 lines)

1. **Purpose.** Turns a call to a toolchain "declaration description" query (`Callable`, `Schema`, `Field`, `Parameter`) into a constant record value built from bytes the source compiler computed. No callee body is instantiated.
2. **Shape.** `declaration_query` (:447, entry, returns `Option<Expression>`), `schema_description` (:326), `field_description` (:177), `parameter_description` (:25), `description_result` (:158), `invalid` (:15, refusal with `RefusalReason::ForeignIdentity`).
3. **Inputs/outputs.** Reads `typed.core_vocabulary`, `declaration_operand_from_expression`, `source_state::reflection::construction::{parameter_description_identity, field_description_identity, callable_description_identity, schema_...}`; produces `ExpressionKind::Record` / `Case` of `byte_literal`s and integer/boolean fields.
4. **How it decides.** Result record fields are found by **string compare of `field.identity == name`** against fixed names such as `"requirement_identity"`, `"declaration_position"`, `"source_revision"` (:109, :125, :251, :284, :603). The result type is checked by `core_vocabulary(...) == Some(Reflection(ParameterInfo|FieldInfo|TypeSchema|CallableInfo))` (:74, :231, :413, :558); case chosen by `case.identity == "Requirement" | "Machine"` (:573-583).
5. **Cases by shape.** Fixed field rosters: "6" fields for ParameterInfo (:90), "11" for FieldInfo (:243), exactly one field `"storage"` for TypeSchema (:427-430), exactly 3 identity fields for CallableInfo (:592-618). Any other roster is refused. Integer fields must be `u64` non-address unqualified (:130-133, :293).
6. **Same job elsewhere.** The three description builders repeat the same "find one field by identity, error if absent or repeated" loop (:106-114, :248-256, :281-290, :600-610).
7. **Unused/one-caller.** `declaration_query` grepped: 1 call, `read/calls.rs:823`.
8. **Stated limits.** none stated.
9. **Fail-open.** `declaration_query` returns `Ok(None)` when the callee is not a reflection query (:458-468); "traced": lets an ordinary call proceed to the normal path. Not accepting a wrong program.

---

## 4. read/regions.rs (880 lines)

1. **Purpose.** Owns "regions" (erased lifetimes) in the reader: what a `Region` is, region variables with ceilings, substitution at record field reads and calls, call-site unification, elision rules, and the comparison `agrees`/`same_type` that decides whether two value types are the same including regions.
2. **Shape.** `enum Region { Parameter, Fixed, Storage, Variable }` (:50), `RegionVariable { decided, within, peer }` (:91), `enum Elision { Declaration, Members, Inputs, Result, Variables }` (:425), `Program::decide` (:134), `same_type` (:593), `agrees` (:575), `unify`/`unify_regions` (:338, :357), `substitute` (:260), `field_regions` (:302), `same_signature` (:684), `borrowed_view_region` (:738), `argument_context` (:857), `elided_region` (:460).
3. **Inputs/outputs.** Reads `region_variables`, `region_tuples`, `fixed_regions`, `elided_lifetimes`, `typed.lifetime_declaration_owner`. Mutates those tables. `same_type`/`same_region` may decide variables and roll back by cloning the whole `region_variables` vec (:608, :665).
4. **How it decides.** Regions are interned by tuple equality (linear scan, :213); fixed regions by **text identity string** `known == identity` (:222-225); parameters by `(declaration symbol, ordinal)`. `agrees` first returns true for a type that "excludes values" (:580-584).
5. **Cases by shape.** `Elision::Result` accepts exactly one or zero input lifetimes, else refuses (:469-479). Members elision always refuses (:463-467).
6. **Same job elsewhere.** `same_signature` here (:684) vs `read/machine.rs:1060, 2535` and `lib.rs:877`, `read/calls.rs:1645` callers (grepped; 5 non-test call sites).
7. **Unused/one-caller.** `same_signature`: 5 non-test calls (grep above). Others are `pub(crate)`; not individually grepped.
8. **Stated limits.** Doc at :26 says "Nothing here orders two named regions".
9. **Fail-open.** `agrees` returns `true` when `type_excludes_values(actual)` (:580-584), the comment at :571-573 says "An empty result fits any destination ... because evaluation produces no value"; "stated". `type_excludes_values` returns `false` (not accepting) when inhabitation metadata is missing (:630-637, stated at :634-635). `same_type` can mutate variable decisions as a side effect of a read-only-looking compare; rolled back on failure only (:620-623).

---

## 5. read/regions/result_lifetimes.rs (539 lines)

1. **Purpose.** Computes the declared lifetime relations of a public signature (which input lifetimes may flow to which result paths, and which writable descriptors may receive which stored lifetimes) as published `DeclaredResultLifetime` and `BoundaryReferenceOutput` rows. They state possibilities, not what the body does.
2. **Shape.** `declared_result_lifetimes` (:27, returns `Checked<Option<Vec<...>>>`, `None` meaning "unresolved frontier"), `declared_reference_outputs` (:92), `struct Frontier` (:272) with `inputs` (:280), `region` (:333), `value` (:352), `record` (:377), `fields` (:517); `region_key` (:244).
3. **Inputs/outputs.** Reads `Signature`, canonical `StructuralTypeDeclaration`s, `Program::data_type`, `source_declaration`, `own_regions`. Returns groups; mutates nothing.
4. **How it decides.** Groups by `region_key(Region)` tuple (:244-270) with sorted binary search; fields keyed on `canonical.identity` ids and skipping `canonical.relevance.is_erased()` (:530).
5. **Cases by shape.** `Frontier::record` matches `StorageShape`: `TypeParameter` -> unknown (`false`), `Reference`, `ByteSequence(BorrowedView|BoundedView)`, `Opaque` (`regions.is_empty()`), `PrimitiveScalar` -> known (:397-431), then `FixedArray` (length 0 is known), `ElementView`, `Record`, `Sum`; the rest `unreachable!` (:511). A recursive type (`self.active.contains`) returns `false` (:385).
6. **Same job elsewhere.** none seen in the unit.
7. **Unused/one-caller.** `declared_result_lifetimes`: grepped, 4 production callers (`boundaries.rs:109`, `callables.rs:163`, `declare_selected.rs:388`, `read/review_requirements.rs:174`) plus `reference_results.rs:98`. `declared_reference_outputs`: 1 caller, `boundaries.rs:269`.
8. **Stated limits.** "`None` preserves an unresolved frontier. In particular it is never a complete empty result contract" (:25-26).
9. **Fail-open.** `ValueType::Scalar => Ok(true)` (:360), `PrimitiveScalar | ByteSequence(_) => Ok(true)` (:427-429): a scalar or a non-view byte sequence carries no lifetime; traced as correct by the shape. `Opaque` with empty regions returns true (:426). None found where unknown counts as "no lifetimes": unknown returns `false`/`None`.

---

## 6. read/rejections.rs (703 lines; code 1-513, tests 515-703)

1. **Purpose.** The reader's single place that turns a "value of type X supplied where Y is required" mismatch into either a spec-defined rejection (`reject`) or a plain "not supported yet" refusal.
2. **Shape.** `enum Destination { Parameter, Result, MatchArm, Field, Local, Assigned }` (:61), `Program::mistyped` (:91), `different_static_arguments` (:222), `different_const_values` (:321), `names_own_storage_as_declared_lifetime` (:353), `names_two_own_lifetimes` (:378), `domains_named` (:395), `domain_named` (:417), free fns `implicit_dereference` (:454), `membership_carrier` (:473), `scalar_named` (:493).
3. **Inputs/outputs.** Reads `self.records`, `record_sources`, `typed.unique_data_definition_by_symbol`, `normalized_nominal_type_identity`, regions. Returns a `Refusal`. Mutates nothing.
4. **How it decides.** The decision order in `mistyped` (:91-217): array lengths (:110), scalar carrier inequality (:122), domain sets with same erased carrier (:137-152), static arguments (:153), nominal declaration identity (:175-189), own-storage-as-declared-lifetime (:190), two own lifetimes of one machine (:204). Keyed on `ScalarType` equality, `Qualification` equality, normalized nominal identity, region tuples. The message text uses `display_path(...)` and `scalar_named(...)` strings, but the decision does not read them.
5. **Cases by shape.** Rejection only when the rule "defines the error for every program of that form" (header comment :1-49); everything else stays `refuse`. `different_static_arguments` only handles `TypeParameterKind::Implementation` and `Const` (:288-316); other parameter kinds skipped (`_ => {}`).
6. **Same job elsewhere.** `scalar_named` is a display formatter; the same width formatting for `IntegerWidth::Parameter` appears nowhere else in unit. Test at :522-540.
7. **Unused/one-caller.** `mistyped`: grepped `\.mistyped\(` count 11 across 7 files (calls.rs 1, expressions.rs 1, machine.rs 1, places.rs 1, rejections.rs 4 (3 are tests), statements.rs 2, transitions.rs 1).
8. **Stated limits.** "Any other difference of regions ... stays a refusal" (:40-41).
9. **Fail-open.** None seen: the function returns a refusal in every path (`refused` at :216); `different_static_arguments` returning `None` falls through to the refusal. `names_*` helpers return `false` on missing symbol (:359-361), leaving the plain refusal (traced).

---

## 7. read/review_requirements.rs (259 lines)

1. **Purpose.** For each public or boundary trait requirement, collects the contract the package manager reviews later (parameter types, service reach, invocation list, termination, crash routes, which parameters a result may borrow from, domain qualifications) into `RequirementReviewMetadata`, or an error string.
2. **Shape.** `Program::review_requirements` (:17), `read_public_requirement_contract` (:70). Output type `RequirementReview` (`publication.rs`).
3. **Inputs/outputs.** Reads `static_contracts.schemas`, `declared_requirement_headers()`, `static_envelope`, `declared_result_lifetimes`, `module.machine(machine)`. Returns `Vec<RequirementReview>`; errors are stored as `Err(String)` inside `metadata`, not refused.
4. **How it decides.** Selects public/boundary by `owner.is_public || owner.is_boundary` (:52). Requirement identity by `declaration` symbol. Termination by `envelope.termination` matching `NotGuaranteed` or `Terminates{premises empty}` (:145-153).
5. **Cases by shape.** Returns `Err` (review impossible) for: a trait with lifetime parameters, a requirement with type parameters, conformance bounds, native callback parameters or default body (:79-89); any result lifetime group with non-empty paths (:183-187) or empty inputs (:188-193); input path non-empty (:196-208); a domain with type parameters or index arguments (:234-239). Comment at :168-170: "Some headers publish reference groups during lowering."
6. **Same job elsewhere.** `service` closure (:106-125) duplicates `applied_source_service` logic of `services.rs:1459` in a different form (inferred, read both).
7. **Unused/one-caller.** `review_requirements`: 1 call, `lib.rs:1374` (grepped).
8. **Stated limits.** "public requirement review needs its complete static/default/callback contract" (:86); "needs projected result lifetime paths" (:185).
9. **Fail-open.** `.filter_map(...)` at :51 `?` on `trait_machine_signature(declaration)` silently skips a declaration with no trait row (returns `None`), so it produces no review row. Open: no stated reason; a requirement whose trait lookup fails is omitted from review rather than refused. Also `if !owner.is_public && !owner.is_boundary { return None }` skips private ones, stated by design.

---

## 8. read/services.rs (1720 lines)

1. **Purpose.** Computes each callable's operational envelope (service reach, `invokes` entries, suspends, blocks, termination) in two parts: the authored ceiling and what the body actually reaches, then publishes a service map. Also reads `invokes` header rows, service identities and awaited providers.
2. **Shape.** Free fns `unavailable` (:25, the shared `Unsupported` refusal used across the crate), `service` (:34), `finite_reach` (:54), `invocations` (:75), `termination` (:130). `Program::signature_envelope` (:149), `finish_operational_envelopes` (:186, the fixed-point), `declaration_axes` (:647), `declaration_policies` (:675), `retain_requirement_policy` (:722), `retain_requirement_schema` (:777), `prepare_service_bindings` (:1270), `service_map` (:1302), `service` (:1406), `declared_service` (:1478), `reach_ceiling` (:1503), `application_binders` (:1526), `entry_ceiling` (:1588), `awaits` (:1667), `publish_readiness` (:1678). Types `KnownAxes` (:910), `Policy` (:957), `Use` (:1005), `CallCollector` (:1014) with `collect_statement`/`collect_expression` (:1095, :1142).
3. **Inputs/outputs.** Reads bodies (`IdTable<MachineId, Body>`), `Components`, `callables.envelopes/contracts`, `declaration_*` tables, `boundaries`, `external_providers`. Mutates `signatures[..].operational`, `callables.bind_machine_envelope`, `declared_envelope_policies`, `checking_envelopes`, `services`, `readiness`.
4. **How it decides.** Fixed-point loop (:300-508) with change flags over `published`/`known`/`declaration_envelopes`. Keyed on `MachineId`, `ContractId`, `ServiceReachId`. A callee's reach is a `CallTarget` variant: `Machine`, `Selected{contract}`, `Boundary` (:313-364). `KnownAxes` booleans say whether an axis is inferred or open. `Policy::complete` keeps authored bounds even when a call exceeds them (:975-984, comment :975-976 "The lowered-form checker rejects that actual call").
5. **Cases by shape.** `CallCollector` hand-lists every `ExpressionKind` (:1142-1266). `finite_reach` follows `abstract_bound` rows with a cycle check (:54-73). `invocations` accepts `Parameter` / `AppliedService` / `Service` targets only (:83-122). Refuses: termination guarantee with non-empty premises (:141-145), recursive edge with static callback arguments (:288-297), nested callback invocation parameter (:429-434).
6. **Same job elsewhere.** `source_roots` (:1387) and `StaticSourceEnvelope::service_roots` (`static_contracts.rs:67`) do the same extraction of service ids from an envelope (read both).
7. **Unused/one-caller.** `finish_operational_envelopes` 3 non-test call sites (`lib.rs:1150, 1157`, test `record_parameters.rs:946`); `service_map` 2 (`lib.rs:982, 1174`); `unavailable` is used across the crate (e.g. `read/types.rs`, `read/storage_metadata.rs`, `read/traits.rs`, `read/source_records/layouts.rs`) (read, not counted).
8. **Stated limits.** "callable termination premises need retained lowered progress subjects" (:143); "nested invocation permission needs the invocation's exact actual mapping" (:432).
9. **Fail-open.** (a) `call_terminates` for `Machine` target uses `named.get(*machine) == Some(&true)`, a missing row is `false` (:884); safe. (b) Termination: `local_termination` starts true for any `private` machine not reaching an unranked cycle (:536-542) "traced": private machine without a termination guarantee is treated as terminating if its call components are ranked; this is the comment at :526-528 ("A private body's progress belongs to this checked closure"), stated. (c) `Policy::initial` gives `unwrap_or(false)` for suspends/blocks (:969-970), used only as a seed before the fixed point; open. (d) CallCollector: an exclusive `Borrow` of a callback slot clears its parameter binding (:1201-1206) rather than refusing; stated ("An exclusive borrower can replace the callback before its next read").

---

## 9. read/source_operators.rs (772 lines)

1. **Purpose.** After the reader successfully forms a builtin operation, records that the operator-selection question attached to it has been answered by the builtin. It also finds the exact operand type references for the operator lookup.
2. **Shape.** `complete_builtin_operator` (:399), `complete_operator` (:409), `complete_formed_operator` (:291), `complete_type_equation` (:90), `complete_anonymous_operators` (:99), `skip_vacuous_disjuncts` (:35), `read_static_truth` (:64), `operator_operand_reference` (:113), `structural_operator_operand_reference` (:169), `complete_collection_range_operator` (:257), `complete_executable_range` (:270), `validate_core_operator_application` (:555), `complete_typed_contract_operator` (:605), `operator_owner` (:22).
3. **Inputs/outputs.** Reads `typed.authored_declaration_selections()`, `expression_table`, `source_state::operator::*`. Writes `self.completed_operators.record_builtin/record_vacuous`.
4. **How it decides.** Keyed on the authored selection occurrence's `kind()` and `target()` (:460-480): `LateBound(CheckedOperator)` pending; `Intrinsic(BuiltinOperator)` ok; `Intrinsic(CoreIntegerOperator(app))` validated against operand signature; any other target is an error (:474-478). Operator kind comes from `binary_operator_spelling(binary.operator)` (an enum mapping, not text).
5. **Cases by shape.** `complete_typed_contract_operator` handles `Name`, `Boolean`, `Call` (only compiler-primitive bodyless headers, :656-675), `Binary` (only `Equal|NotEqual` over equal Boolean/Integer scalars, `And|Or` over Boolean, :728-743), `Unary(LogicalNot)`; all else `Ok(None)` (:769). "typed contract has an unformed source operator" otherwise (:745).
6. **Same job elsewhere.** none.
7. **Unused/one-caller.** Each is `pub(super)`; not grepped individually.
8. **Stated limits.** "Unknown collections and selected user operators stay pending" (:502).
9. **Fail-open.** `complete_operator` returns `Ok(())` early for non-`Binary/Unary/Indexed` expressions (:445) and when `!pending` (:482-484), and returns `Ok(())` (without recording) when a bare operation has no builtin meaning (:538-548) or an index operand is unknown (:497-512). "stated" at :545-546 ("A declaration or abstract operator requirement still needs its actual selection"). Effect: the question stays unanswered, and the later `Capture` (`selection_questions.rs:1043-1092`) fails with "remains unresolved as CheckedOperator" (traced). So skipping does not accept a program by itself.

---

## 10. read/source_records.rs (1850 lines; code 1-1232, tests 1234-1850)

1. **Purpose.** Keeps each nominal record's source identities (declaration, fields, cases, applied type arguments, layout plan) alongside the structural row, validates that an application matches its declaration, and publishes the correspondence and source declaration identities into the module.
2. **Shape.** `semantic_reference` (:24), `source_application` / `source_application_in_scope` (:57, :66), `validate_applied_members` (:320), `same_application` (:397), `Program::record_application` (:464), `publish_source_declarations` (:511), `access_plan_regions` (:677), `source_record_correspondence` (:822), `malformed` (:1149, `ForeignIdentity` refusal), `member_rosters_match` (:1158), `correspondence` (:1168).
3. **Inputs/outputs.** Reads `data_application_definition`, `closed_data_definition`, `layout_storage_arguments`, `plan_laid_layout_for_type`, `records`, `record_sources`. Writes `module.types.structural_types[..].declaration_identity`, case/field `declaration_identity`, `module.identities`.
4. **How it decides.** Application identity by `typed.normalized_type_identity` equality (:113, :404); template binders by symbol and ordinal (:177-181); member roster by symbol/identity/name/relevance (:332-353, :360-384); storage shape drives which correspondence rules apply in `source_record_correspondence` (:837-1079).
5. **Cases by shape.** `source_application_in_scope` refuses: type binders unresolved (:94), type plus lifetime arguments (:135), generic premises/bounds (:140), cleanup or quotient (:145), constrained parameters (:190), parameter kinds other than Type/Const (:261), const argument with unsupported carrier (:220). Refuses reference arguments whose regions are not retained (:277). Comment :1059-1062: "These physical shapes have no nominal declaration or field schema" for `FixedArray | PrimitiveScalar | AtomicCell`.
6. **Same job elsewhere.** `same_application` (:397) vs `SourceTypeApplication::type_identity` (outside unit). `access_plan_regions` and `publish_source_declarations` both walk `source.fields` and `source.cases` with `exact_symbol_identity` (:610-618, :752-756, :776-783).
7. **Unused/one-caller.** grepped: `publish_source_declarations` 1 caller (`lib.rs:1660`); `access_plan_regions` 1 (`qualifications.rs:140`); `record_application` not grepped.
8. **Stated limits.** "record application with unresolved type binders" (:96); "unlowered cleanup or quotient premises" (:151).
9. **Fail-open.** `semantic_reference` returns `Ok(storage.semantic_type)` when a storage application has no layout plan (`(Some(storage), None)`, :49), stated "Its target geometry is required only when a consumer asks for it". `same_application` returns `true` when there are no substitutions (:409-411); traced as correct for a non-generic type. `source_record_correspondence` `continue`s (no row) for TypeParameter/ByteSequence/Reference/ElementView/FixedArray/PrimitiveScalar/AtomicCell shapes after validating each (:837-1079); stated "cannot mint nominal source authority".

Tests (1234-1850, read): fixtures for applications with const/type arguments, forged members, aliases.

---

## 11. read/source_records/layouts.rs (490 lines)

1. **Purpose.** When a record uses a layout policy, publishes the target layout applications and native layout catalog (per target pointer size, field offsets, byte size) into the module.
2. **Shape.** `publish_record_layouts` (:32), `target_geometry` (:255), `field_coordinates` (:336), `field` (:356), `realization` (:371), `TargetGeometry` (:247).
3. **Inputs/outputs.** Reads `typed.plan_laid_layouts`, `plan_laid_target_layouts`, `layout_storage_arguments`, `record_sources`, `module.layouts.native_layouts`. Writes `module.layouts.target_layout_applications`, `native_layouts`, `identities`, changes record `storage` to `TargetLayout(id)` (:227).
4. **How it decides.** Plans joined to records by `normalized_type_identity` using sorted vec + `binary_search` (:48-54, :128-134); fields joined by symbol `(arena_index, generation)` (:349, :362); targets sorted by `target.as_str()` (a string, :274, :287) and matched pairwise.
5. **Cases by shape.** Requires every layout plan to carry `PlanLaidRealizations::TargetInputs` (:262), non-empty target roster, equal counts (:288-298), fixed offsets and complete extent (:391-396, :481-486); field kinds Scalar/Nested/Repeated (:425-435). Refuses a semantic schema that is not unique (:154). Policy mismatch refuses (:129-150) using `LayoutPolicyError::WrongSemanticType` -> `reject` and `MissingEvidence` -> `malformed` (:111-126).
6. **Same job elsewhere.** none.
7. **Unused/one-caller.** `publish_record_layouts`: 1 caller, `lib.rs:1263` (grepped).
8. **Stated limits.** none.
9. **Fail-open.** If a previous native layout record has no `layout_storage_arguments` (`storage` is `None`), it is copied through unchanged (:89-107). Traced to old catalog entries; accepted without re-check; the comment gives no reason. Early return `Ok(())` when no plans and no `authored_layout` record (:38-45).

---

## 12. read/source_records/opaque.rs (298 lines)

1. **Purpose.** After the program is read, replaces each opaque (boundary-opaque) record's shape with its selected carrier's storage if the opaque type is passed by value in some ABI or place, keeping an `opaque_representations` relation.
2. **Shape.** `OpaqueCarrier` (:21), `Program::prepare_opaque_carriers` (:28), `publish_opaque_carriers` (:74), `by_value_roots` (:189), `publish_native_layouts` (:249).
3. **Inputs/outputs.** Reads `typed.opaque_representation_selections`, `record_sources`, module machines/contracts/ABI signatures. Writes `module.types.structural_types[..]` (shape, scalar restriction), `module.types.opaque_representations`, layout applications, `native_layouts`.
4. **How it decides.** Carriers matched to occurrences by `binary_search` over selections keyed on `(arena_index, generation)` of the opaque symbol (:36-44). A carrier is published only if `demand.binary_search(&opaque).is_ok()` (:99): `demand` is storage reachable from by-value roots (owned parameters, structural places, results, ABI storage).
5. **Cases by shape.** Refuses if the opaque declaration shape is not `Opaque` (:165-170) or carrier shares the opaque id (:57-64).
6. **Same job elsewhere.** none.
7. **Unused/one-caller.** `prepare_opaque_carriers` 1 non-test caller `lib.rs:1169`; `publish_opaque_carriers` 1 caller `lib.rs:1363` (grepped).
8. **Stated limits.** "Expressions only see the opaque declaration" (:1-3).
9. **Fail-open.** `continue` when an opaque type is not in `demand` (:99-101): it stays opaque, which is stated by the module comment and is the intended default. None else.

### 12a. read/source_records/opaque/tests.rs (285 lines)
Test only: builds one opaque `Receipt` with a `Word` carrier and checks by-value ABI publishes carrier bytes, by-reference does not, and the codec rejects a redirected carrier. Read in full.

---

## 13. read/source_views.rs (53 lines)

1. **Purpose.** One entry that resolves a type reference to a slice application once and chooses between a byte view and a typed element view.
2. **Shape.** `SourceViewKind { Bytes, Elements }` (:9), `SourceView` (:14), `source_view` (:20).
3. **Inputs/outputs.** Reads `source.scope.slice_application`, `super::bytes::source_byte_view`, `super::arrays::source_sequence_view`. Returns `Option<SourceView>`.
4. **How it decides.** Tries bytes first, then elements; neither returns `None` (:39-47). Judgment is the callee answers.
5. **Cases by shape.** two kinds only.
6. **Same job elsewhere.** none.
7. **Unused/one-caller.** 1 caller seen in this unit (`source_records.rs:983`); other callers suspected in `read/arrays.rs`/`read/bytes.rs` (not read).
8. **Stated limits.** none.
9. **Fail-open.** `Ok(None)` when no slice application or neither view kind matches (:36-37, :46); callers then treat the type as not a view. Traced at `source_records.rs:983-994`: the caller refuses with "no exact source view declaration" for ElementView storage.

---

## 14. read/statements.rs (753 lines)

1. **Purpose.** Reads statement forms of a body: evaluate, call statement, assignment (to name, member/index/deref place), local binding (with or without initializer, reference capture, erased), and block values.
2. **Shape.** `evaluated_expression` (:29), `call_statement_at` (:44), `require_whole_replacement` (:71), `assignment` (:82), `local` (:327), `next_slot` (:573), `adopt_block_slots` (:584), `block_value` (:601), `block_statements` (:623), `erased_local` (:702).
3. **Inputs/outputs.** Reads `typed.statement_table`, `expression_table`, `Slot` array; returns `Statement` and `Slot`. Mutates `self.block_slots`, `self.block_floor`, `self.names`.
4. **How it decides.** Slot properties `assignable`, `homed`, `borrowed`, `erased` (:113, :163, :185-187). Assignment target by `ExpressionNode` variant (:130-324). Binding role by `local.role` enum (:334-349).
5. **Cases by shape.** List of refusals: `AtomicObservedResult`, `RecordPattern`, `ArmPattern`, `DispatchObservation` locals (:337-348); `reflected_query_origin` (:357); non-`Name/Member/Indexed/Deref` assignment targets "assignment to a projected place" (:304-323); "assignment inside a block value to a scalar local declared outside it" (:190-194, the TASKS item at `TASKS.md:25`); "uninitialized local of a qualified type" (:461); "local bound to a reference without a declared type" (:523-528); block statements: transitions/assembly refused (:675-681).
6. **Same job elsewhere.** `block_statements` (:623-697) is a second statement dispatcher beside the body driver in `read/machine.rs` (suspected, not read).
7. **Unused/one-caller.** not grepped; all `pub(super)`.
8. **Stated limits.** `unserved("a block value outside a body statement")` (:613); header comment lists supported assignment forms (:1-12).
9. **Fail-open.** `local` ordering: the `declared` type is checked with `agrees` before binding (:534-545); a declared-but-differently-regioned type replaces the initializer's type when erased carriers differ (:549-551) — "A bottom initializer produces no value, but the declared local keeps its annotation" (stated, :546-548). `let x: T;` is zero-established (`zero_established`, :456), stated: "This is not an assumed zero for genuinely uninitialized storage" (:440-443).

---

## 15. read/static_contexts.rs (214 lines)

1. **Purpose.** Rewrites static-callback bindings on copied helper bodies so each call refers to the original callback parameter of its declaring machine, and returns the list of `StaticReachParameter` rows.
2. **Shape.** `static_reach_contexts` (:42), `static_parameter` (:28), `Origin` (:14).
3. **Inputs/outputs.** Reads `self.static_reach_parameters`, `typed.declaration_type_parameters`, source call correspondences. Mutates `module.machines[..].blocks[..].operations[..].static_reach_binding` and `InvokeNoNormal` terminators (:174-209).
4. **How it decides.** Keyed on `(owner MachineId, parameter symbol key)` and `(machine, site)` of the call correspondence via `binary_search` (:136, :158); repeated sites refused (:126-132).
5. **Cases by shape.** A binding on a non-call operation is refused (:180-185). A binding for a requirement not in `requirements` is left unchanged (`if requirements.binary_search(&inputs.callee).is_ok()`, :186, :201).
6. **Same job elsewhere.** none.
7. **Unused/one-caller.** `static_reach_contexts`: 1 caller, `lib.rs:1272` (grepped).
8. **Stated limits.** none.
9. **Fail-open.** A static binding whose callee is not a known requirement is not normalized and left as is (:186, :201). Open: no comment gives a reason; a wrong program would carry an un-normalized binding to later stages. Early `Ok(Vec::new())` when no parameters (:47-49).

---

## 16. read/storage_metadata.rs (264 lines)

1. **Purpose.** Describes the ABI storage (parameter and result carrier types) of each selected calling signature after all bodies are read, and binds the substitutions of a calling application.
2. **Shape.** `enum TypePurpose { Value, StorageDescription }` (:15), `PreparedCallingDescriptions` (:21), `bind_calling_storage_substitutions` (:29), `prepare_calling_storage` (:91).
3. **Inputs/outputs.** Reads `SourceCallingPolicies::signatures`, `projections_in_context`, `calling_abi_parameter`, `calling_abi_storage`. Returns `PreparedCallingStorage` rows.
4. **How it decides.** Keyed on ABI key identity: every projection of one key must produce equal `storage` (`agreed` compare, :234-240) and equal `formals`, `default_eligible`, `physical_owner` (:129-132).
5. **Cases by shape.** Refuses telescopes with binders or direct callbacks "calling metadata needs its exact published callback catalog" (:142-147). Result `Unit` handled by `omitted_unit` check on `trait_machine_signature`/entry state (:178-189).
6. **Same job elsewhere.** none.
7. **Unused/one-caller.** `prepare_calling_storage` 1 non-test caller `lib.rs:1167`, plus tests at `read/calling.rs:1277, 1343` (grepped).
8. **Stated limits.** "calling storage before the complete body closure" (:98).
9. **Fail-open.** none seen.

---

## 17. read/storage_path.rs (245 lines)

1. **Purpose.** One walk from a place expression to its storage root, independent of the operation, with a callback per projection and capture resolution.
2. **Shape.** `walk` (:15), `Captures` and `resolve` (:70, :85), `root` (:102), `root_slot` (:111), `names_storage` (:124).
3. **Inputs/outputs.** Reads `Expression` trees; returns the root expression. Pure.
4. **How it decides.** Projection kinds `Field | Payload | Index | ByteElement | Referent` continue; `SavedSubject` resolves through captures; every other `ExpressionKind` (listed explicitly :34-63) stops and returns the place.
5. **Cases by shape.** Cycle bound `followed == subjects.len()` -> "cyclic saved subject capture" (:87-89); no match -> "outside its match capture" (:93); more than one -> "ambiguous" (:95).
6. **Same job elsewhere.** none.
7. **Unused/one-caller.** `names_storage`, `root_slot`, `root`: not grepped (all `pub(super)`); `walk` used by `references.rs:428, 497` and `statements.rs:264` (read).
8. **Stated limits.** none.
9. **Fail-open.** `walk` returns the place unchanged for any non-projection kind (:63); callers must check the root kind. `names_storage` with `|_| Ok(None)` capture closure treats a `SavedSubject` as storage (:125-131); stated "A saved subject already denotes captured storage".

---

## 18. read/subjects.rs (1230 lines)

1. **Purpose.** Decides which declarations a compilation is responsible for beyond the machines its entries call (every declaration of the subject packages) and reads each, or refuses the kinds it does not judge.
2. **Shape.** `Owner { Subject, Dependency }` (:52), `Program::owner` (:59), `build_entry_context` (:83), `selected_declarations` (:137), `applied_boundaries` (:211), `supplied_slots` (:232), `subject_declarations` (:378), `keep` (:668), `subject_data_template` (:706), `template_type` (:764), `generic_data` (:889), `generic_type_alias` (:922), `alias_expansion` (:968), `generic_trait` (:1000); free fns `served_trait_header` (:1062), `served_trait_parameters` (:1081), `served_parent` (:1144), `names_only` (:1177), `judged_where_used` (:1205), `symbols_of` (:1225).
3. **Inputs/outputs.** Reads every typed declaration table (`machines()`, `data_definitions()`, `domain_definitions()`, `traits()`, `conformances()`, `const_declarations()`, `mathematical_definitions()`, `type_aliases()`, `measures()`, `operators()`, `propositions()`, `wire_schemas()`). Mutates `admitted_declarations`, `subject_machines`, `generic_templates`, `fixed_contracts`, `conformance_witnesses`, `supplied`, `collected`.
4. **How it decides.** Subject membership via `source_state::source_subjects::contains(typed, symbol, subjects, purpose)` from the source file the symbol was declared in, "never from a path or a name" (:22). Machine kind: `machine.purpose.is_active_case_visitor()`, `machine_instantiation(..).is_some()` skip (:413-433). `keep` records a refusal and continues when `collected` is set (:668-676).
5. **Cases by shape.** Hand-listed declaration tables (:404-660). Four kinds refused if they are subject declarations: `measure`, `operator`, `proposition`, `wire schema` with an encoding (:633-660). Generic data accepted only with: `CheckedShape`, no lifetimes/bounds/cleanup/quotient/retired members, unbounded Type or supported-Const parameters, relevant members, type forms Named/Reference/FixedArray/Constrained/Generic, depth <= 64 (`template_type`, :784). Domain accepted only if `judged_where_used` (no alias/cleanup/operators, direct index telescope, Type or Const params) (:1205-1223). Trait header: no lifetime parameters, unbounded types, implementation parameter with exactly one closed bound (:1081-1126), no transparent refinement (:1067).
6. **Same job elsewhere.** `supplied_slots` conformance witness push (:336-369) vs `read/traits.rs:676-683` (`generic_conformance_witnesses`): same `ConformanceWitness` construction and `contains` guard (read both).
7. **Unused/one-caller.** grepped: `build_entry_context` 1 caller (`lib.rs:384`); `generic_data`/`generic_trait`/`generic_type_alias` 1 each (`lib.rs:673, 659, 691`). `served_trait_header` used in `traits.rs:1023, 637, 653`.
8. **Stated limits.** Header: "Every other kind of declaration is refused as `Unsupported`, because leaving an unjudged declaration out would accept it" (:19-20). "A generic machine is set aside for its own check form" (:10). Depth bound comment: "A source type nests far less deeply than this" (:782-783).
9. **Fail-open.** (a) `subject_declarations` returns `Ok(())` when `subjects.is_empty()` (:386-388): stated "A concrete evaluation asks for its existing entry and call closure, not a full check". (b) `names_only` data declarations are skipped as subjects (:471-475), stated at :1171-1176 (no value of the type exists; a use resolves it later). (c) Domain declarations are not judged at declaration, only where used (`judged_where_used`, stated :1190-1204). (d) `conditional_prefix && deferred_build_subject` skips a subject machine (:437-439), stated at :390-392 only in part. (e) A machine in `machine_token_bindings` mismatch is recorded via `keep` (:608-614). (f) When `collected` is `Some`, a refusal is stored and reading goes on (:668-676), but "the compilation is still refused once reading ends" (stated :664-667).

---

## 19. read/traits.rs (1276 lines; code 1-1173, test 1175-1276)

1. **Purpose.** Forms static schemas of traits (ordinary and boundary) and implementation (conformance) questions: each requirement's signature and envelope, each selected row's body, provider candidate, and generic-implementation witnesses.
2. **Shape.** `implementation_members` (:30), `checked_as_template` (:94), `static_envelope` (:153), `trait_declaration` (:207), `boundary_trait_declaration` (:222), `claim_boundary_trait` (:238), `read_trait_schema` (:250), `require_boundary_parent_contract` (:440), `check_boundary_parents` (:456), `prepare_inherited_boundary_contracts` (:513), `is_generic_conformance` (:591), `generic_conformance_witnesses` (:618), `conformance_declaration` (:689), `applied_scope` (:986), `schema_local_shape` (:1018), `validate_references` (:1155), struct `SignatureReferences` (:1078).
3. **Inputs/outputs.** Reads trait/conformance definitions, `AppliedTraitScope`, closed requirement applications, machine specializations. Mutates `static_contracts.{schemas, conformances, formed}`, `declaration_signatures/sources/selections/envelopes`, `generic_templates`, `boundary_parent_contracts`, `fixed_contracts`, `conformance_witnesses`.
4. **How it decides.** Rows by symbol key `(arena_index, generation)` with `binary_search` (:258-265, :699-706). Implementation rows are matched to requirements through a `std::collections::HashMap<SymbolHandle, ...>` (:39) and counted equal to rows (:53-58). `checked_as_template` is true when the requirement has lifetime parameters, a Type parameter, or an array extent depending on a Const parameter (:94-150).
5. **Cases by shape.** `schema_local_shape` requires no lifetime parameters, no refinements, affine type parameters, no duplicates (:1018-1075). `generic_conformance_witnesses` accepts only unbounded Type parameters (comment at :615-617: "Only unbounded type parameters are served. Lifetime binders, bounded or non-type parameters ... stay refused"). Boundary provider candidates only for `SourceImplementation::Machine` (:873-875). `PrivateCallbackSlot` vocabulary takes a separate path (:721-756).
6. **Same job elsewhere.** `witness` construction repeated in `read/subjects.rs:336-369`.
7. **Unused/one-caller.** `conformance_declaration` and `read_trait_schema` have multiple callers (`subjects.rs`, `record_parameters.rs:71`, traits.rs); `is_generic_conformance` 2 (traits.rs:767, subjects.rs:542).
8. **Stated limits.** Comment "generics.md ... a requirement with binders of its own is a template. Its open signature is checked once in its own check form" (:352-355). "static implementation family needs its general binder or region proof" (:646, :713).
9. **Fail-open.** `read_trait_schema` returns `Ok(true)` early if the schema was already formed (:263); idempotence. `read_trait_schema` for a requirement checked as a template pushes a row with `signature: None` and skips lowering (:356-368); stated at :352-355, and the template is read in its own form (traced to `generic_templates`, `lib.rs:659`). Open: `check_boundary_parents` `needs_contract_owner` result is false for a parent boundary with no methods (:490-508), nothing owed.

Test (1175-1276, read): an ordinary parent with a method requires a contract owner.

---

## 20. read/transitions.rs (618 lines)

1. **Purpose.** Reads transition arms of a machine state: guards, value and structural patterns, targets (value, terminal, call), crash exits, and sum dispatch (one guarded arm per case).
2. **Shape.** `transition` (:27), `transition_predicate` (:101), `case_arm` (:204), `names_case` (:217), `empty_dispatch_subject` (:238), `dispatch` (:275), `value_pattern` (:450), `target` (:479), `crash_target` (:602).
3. **Inputs/outputs.** Reads statement-table transitions, patterns, targets; produces `(Option<Expression>, Target)` and `Statement::Dispatch`. Mutates `names.arms`, `names.shadowed_patterns`.
4. **How it decides.** Dispatch recognizes a case arm from pattern head kind (`names_case`): a head that is a `Data` with only fields is a record (not case) by `symbols.get(named).kind == Data` and members all `Field` (:226-235). `same_computation(guard, case_test_of(read, case))` decides guard equals the case test (:363-370). Warning label for an arm is built from a **string** `"the `{label}` arm of a transition"` where label is `"_"` or a Boolean value (:74-94) — only for the warning name, not a decision.
5. **Cases by shape.** Refuses: `continuation`, `proof_selectors` (:42-55, :393-398), tuple pattern (>= 2 patterns, :182-188), "destructuring pattern" when the record has cases (:131-136), "case arm whose subject is not a data local" (:333), "case arm with a guard" unless the guard equals the case test (:369), "crash in a case arm" (:404), "qualified or receiver call" except requirement-qualified `members == 2` (:542-548), "static or evidence call" (:526-532), "dispatch without a case arm" (:431), "pattern with a separate guard and condition" (:474). Header (:262-274): "Every arm completes the machine".
6. **Same job elsewhere.** none.
7. **Unused/one-caller.** not grepped.
8. **Stated limits.** `unserved("continuation")`, `"proof selectors"`.
9. **Fail-open.** In `dispatch`, a case proven empty by inhabitation in an `ActiveCaseDispatcher` is skipped with `taken += 1; continue` (:374-387); stated at :371-373 ("Generated visitation candidates name callbacks for precisely the cases that can occur"). Arms shadowed by an earlier wildcard or full case are recorded as warnings and dropped (:408-417), never refused; stated by the `shadowed_patterns` name (not a code comment).

---

## 21. read/type_equations.rs (198 lines)

1. **Purpose.** Decides a type equation `A == B` where both sides still contain const parameters, by producing a proposition (a conjunction of const-argument equalities) or a definite falsehood.
2. **Shape.** `structural_type_equation` (:65), `equation_operand` (:16), `equation_const_argument` (:40).
3. **Inputs/outputs.** Reads source applications, `clause.bound`, `scalar_type`; returns `Option<Proposition>`; calls `canonical::equal/conjunction`.
4. **How it decides.** Normalized type identity equality (`left_id == right_id`, :120); different generic bases -> `Falsehood` (:162-164); for non-generic distinct closed types -> `Falsehood` only if both closed (:140-143).
5. **Cases by shape.** `None` ("cannot decide") for data with lifetime parameters (:90, :165), types without identity, unknown parameter kind (:192).
6. **Same job elsewhere.** none.
7. **Unused/one-caller.** not grepped.
8. **Stated limits.** Comment at :138-139: "Distinct normalized types are unequal once both sides are closed. Open binders could still receive the same type."
9. **Fail-open.** None: undecided gives `None` (the caller treats it as undecided, inferred). Equal identities `continue` (:120-122) without proposition: correct for equal types.

---

## 22. read/type_parameters.rs (199 lines)

1. **Purpose.** Gives each type parameter a rigid logical identity plus an independent storage carrier row (`StorageShape::TypeParameter`), and exposes bindings.
2. **Shape.** `ParameterStorage` (:16), `rigid_type_scope` (:24), `extended_source_scope` (:41), `parameter_storage` (:58), `generic_storage_bindings` (:146), `same_type_arguments` (:176), `logical_parameter` (:187).
3. **Inputs/outputs.** Reads `typed.declaration_type_parameters`, `package_qualified_type_identity_text`. Mutates `records`, `record_sources`, `rigid_storage`.
4. **How it decides.** Ordinal indexes `rigid_storage` (a `Vec<Option<..>>`, :134-136); a rigid ordinal must keep one declaring symbol (:78).
5. **Cases by shape.** Only `TypeParameterKind::Type` (:91). Multiplicity maps Unrestricted/Affine/Linear (:102-112).
6. **Same job elsewhere.** none.
7. **Unused/one-caller.** `rigid_type_scope` used by `subjects.rs:907, 957`.
8. **Stated limits.** none.
9. **Fail-open.** none seen.

---

## 23. read/types.rs (1856 lines; code 1-1830, test 1832-1856)

1. **Purpose.** The type family of the reader: maps a typed-tree type reference to a `ValueType` (scalar, record, view, reference, array, callable, qualified), evaluates static type propositions (`T == u32`), lands anonymous integer/float literals, and reads domain qualifications.
2. **Shape.** `atom_type` (:74), `scalar_atom` (:113), `Program::domain_theory` (:170), `declared_value_shape` (:254), `scalar_type` (:329), `closed_type_equation` (:461), `guarded_false` (:660), `static_truth` (:679), `clause_truth` (:688), `type_truth` (:705), `type_truth_operators` (:714), `observed_type_truth` (:726), `evaluate_type_truth` (:759), `plain_scalar_storage` (:893), `storage_carrier` (:939), `value_type` (:969), `qualified_type` (:1143), `type_qualification` (:1169), `scalar_domain_cast_type` (:1222), `qualified_scalar_type` (:1250), `named_domain` (:1291), `declared_domain` (:1325), `domain_type_bounds` (:1451), `result_type` (:1498), `wholly_anonymous` (:1526), `anonymous_value` (:1565), `anonymous_float` (:1673), `float_literal` (:1704), `literal` (:1761), `round_float` (:1812); enums `TypeTruth`, `Reading`, `AnonymousLanding`.
3. **Inputs/outputs.** Reads `type_reference_table`, `scope`, `source_scope`, `const_scope`, domain definitions, `closed_numeric::evaluate_anonymous_value`. Mutates `declared_domains`, `admitted_declarations` (in `declared_domain`).
4. **How it decides.** Type kind by `TypeReferenceNode` variant and `builtin_type_atom(symbol)` (identity, not spelling). `value_type` is an ordered chain of `if let` tries (:974-1128): rigid scope substitution, atomic, callable, view, reference, array, named data, generic data, constrained, lifetime-only generic, else scalar. Static truth is by closed builtin type equality (`Closed::Atom/Scalar/Record`, :546-650).
5. **Cases by shape.** `atom_type` refuses atoms with no `PrimitiveType` (slice, syscall result, terminal, anonymous integer, atomics, selector) by a **label string** passed to `Unserved::Type` (:76-86). `scalar_type` refuses `Reference, Constrained, FixedArray, Slice, ConstExpression, Unit` (:381-448). `type_qualification` refuses: second arithmetic policy, policy on non-integer, compiler-owned domain, unnamed domain, more than four declared domains (:1184-1212). `declared_domain` refuses alias, generic/indexed family, cleanup-selecting, refining, domain on another carrier, predicate on laid storage (:1345-1390). Integer width refused if wider than the checker's 128-bit (comment :65-66, `integer` fn :67). `anonymous_value` refuses beyond 64-bit (:1653-1657, "The reader holds integers of at most 64 bits"). Integer literal: only `Exact` domain (:1770).
6. **Same job elsewhere.** The address/F32/F64 `scalar_type` special cases under `TypePurpose::StorageDescription` (:353-376) duplicate `atom_type`'s outcomes for those atoms with a different gate (read). `domain_type_bounds` reads only `[copy]` bound on the carrier parameter (:1464-1475).
7. **Unused/one-caller.** not grepped (all `pub(super)`); `OPEN_CARRIER_FAMILY` const (:48): not grepped.
8. **Stated limits.** `OPEN_CARRIER_FAMILY` doc: "the body would need to be read once per carrier family, which no reading does yet" (:45-47); "Refused by name: `Trapping`... a compiler-owned domain" (:1140-1141).
9. **Fail-open.** (a) `declared_value_shape` returns `Ok(true)`/`Ok(false)` from preflight with the loop bounded by `type_reference_count()+1` (:268); falls to `Err(unserved)` on exhaustion. (b) `closed_type_equation` returns `None` ("cannot decide") in most unknown cases; `static_truth` then treats as not static. (c) `guarded_false` returns `false` for unknown (:665-673). (d) `type_truth` evaluating `ExpressionNode::Name` finds `static_truths` by `resolved_symbol` (:775-785). (e) `named_domain` returns `Ok(())` when the domain has a definition or typing did not mark the name unknown (:1296-1298); stated by the comment :1284-1290 ("left to `declared_domain`, which refuses what it does not read"). (f) `domain_theory` refuses on an open application (:175-185). No path found that accepts an unknown type.

Test (1832-1856, read): every served scalar atom round-trips through `atom_type`/`scalar_atom`.

---

## 24. reference_ends.rs (287 lines; code 1-249, test 251-287)

1. **Purpose.** After lowering, inserts a `ReleaseReference` operation at the last use of each reference descriptor (and the matching write-log "vacated" rows), using the planner in `terminal_semantics::plain_affine`.
2. **Shape.** `planning_refusal` (:16), `install` (:46).
3. **Inputs/outputs.** Reads `terminal_semantics::reference_ends::candidates(module)` and `plain_affine::plan_reference_ends`. Mutates `module.machines[..].blocks[..].operations`, `tables.places/writes/obligations`, `sources.operations`. Relocates `created_at` of obligations that `precede_execution` (:206-234).
4. **How it decides.** Release placement by `(block id, position)` pairs `end.at == at` (:121-132). Refusal reason: `Error::Unsupported` -> `Unsupported` (collected, continue), `Unavailable` -> `Unestablished`, `Malformed` -> `ForeignIdentity` (:28-32, :87-93). A reference end that crosses `Referent` is refused (:164-170).
5. **Cases by shape.** Machines with no candidates skipped (:82). All `Unsupported` plans are collected and the first is returned after all machines were tried (:243-247).
6. **Same job elsewhere.** none.
7. **Unused/one-caller.** `install` 1 caller, `lib.rs:2015` (grepped).
8. **Stated limits.** Test message: "last-use reference disposition needs an established active case" (:279), the TASKS.md OWNED-CLEANUP item.
9. **Fail-open.** `if candidates.is_empty() { continue; }` (:82) skips machines with none; correct. `ends.next().is_some()` after the walk refuses leftover ends (:235-241). None seen.

---

## 25. reference_results.rs (1224 lines)

1. **Purpose.** Publishes the loan-lifetime facts for returned references: result origin maps, retained call loans, successor loans on block edges, and the "lender" column of every loan obligation. It derives contract rows; the checker reconstructs authority.
2. **Shape.** `publish` (:27), `publish_requirements` (:65), `publish_retained_call_loans` (:119), `struct SuccessorLoan` (:259), `publish_successor_loans` (:267), `capture_argument` (:376), `publish_results` (:421), `publish_parents` (:555), `retained_loan_row` (:1024), `publish_payload_children` (:1064), `reference_access` (:1144), `captured_path` (:1157), `formation_position` (:1188), `graph_unavailable` (:14).
3. **Inputs/outputs.** Reads `Elaboration` module and tables, `selections: &[Selected]`, `nominal_calls`; calls many `terminal_semantics` functions (descriptor analysis, `reference_call_lifetimes`, `borrowed_loan_sources`). Mutates `tables.obligations`, `sources.obligations`, `machine.contract.result_lifetimes`, `result.reference_sources`, and loan `lender` fields (:999-1018).
4. **How it decides.** Loan rows matched by exact `(created_at point, access, captured root, path)` (:796-812, :1042-1058). Lender from `BorrowedLoanSources::single()`: `Owner|Incoming|Loan|Provider` or `Selected` (:963-971). A loan row is unique: "has no unique exact Loan row" otherwise (:813-818).
5. **Cases by shape.** `publish_parents` handles loans reached through `Referent`, a view element descriptor, an `Incoming` lender with a block-parameter root, or created at an `Edge`; other incoming loans `continue` (:599-637). Only `EstablishReference | EstablishElementView | EstablishMutableElementView` operations are formation points (:768-772).
6. **Same job elsewhere.** `reference_access` here (:1144) vs the `StructuralAccess -> ReferenceAccess` inline conversions in `references.rs:234-245`, `result_lifetimes.rs:296-300` (read).
7. **Unused/one-caller.** `publish`: 2 call sites in `lib.rs` (:1352, :1996); `publish_requirements`: 1 (`lib.rs:1347`) (grepped).
8. **Stated limits.** none besides the header ("It derives a contract, never its loan or authority").
9. **Fail-open.** In `publish_parents`: for a `Lender::Incoming` loan whose root is not a `BlockParameter` and whose point is not an `Edge`, the loop does `continue` leaving the lender `Incoming` (:631-637). Stated at :620-622 ("An invocation input's parent belongs to its caller"). `publish_retained_call_loans` skips callers when none return borrowed (:152-154). In `publish_payload_children`, a formal that is not a borrowed-view payload `continue`s (:1094-1099), stated at :1062-1063.

---

## 26. selection_layout_query_tests.rs (126 lines)

Test only, mounted at `selection_questions.rs:1360-1362`. For `Size` and `Alignment` layout queries: capture keeps the data operand as a dependency and refuses a stale target symbol, a missing target, and a stale operand symbol ("missing or stale declaration coordinate"). Read in full.

---

## 27. selection_questions.rs (2235 lines; code 1-1358, tests 1360-2235)

1. **Purpose.** Walks the admitted source with the typed-reference visitor and records each authored declaration-selection occurrence (calls, operators, member access, static arguments, Build designations, computed calls) as a `RawSelectionQuestion`, plus declaration dependencies and formation owners. It grants no permission; its output is the source question list for stage 06/07.
2. **Shape.** `CompletedOperators` (:25) with `record_builtin` (:64), `record_vacuous` (:142), `finish` (:146), `contains` (:159); `bind_invocation_declarations` (:186); `SelectionCaptureError` (:226); private `Capture` struct (:265) implementing `TypedReferences` (:804-1196: `declaration`, `expression`, `statement`, `authored_selection` (:987), `callable_use`, `static_application`, `product_type_input`, ...); `capture` (:1199, test-only), `capture_with_bodies` (:1220); `declaration_custody` (:1352); `formed_product_type_inputs` (:313).
3. **Inputs/outputs.** Reads `TypedTrees` (selections ledger, expression/statement tables, symbols), `DeclarationSelection` admitted and bodies rosters, `CompletedOperators`, signature views. Produces `RawSelectionQuestions { questions, dependencies, declarations }`.
4. **How it decides.** Authored operator questions are answered only by an exact `(owner symbol, expression handle, occurrence id)` triple in `CompletedOperators.reads` (:159-176), a sorted vec with `binary_search_by_key` (not a map). `Execution` vs `ProductDesignation` role by presence of a designation for the occurrence (:1007-1011). Local bindings by `SymbolKind` list (:497-506). Dedupe via `HashSet` with `arena::BuildIndexHasher` keyed on tuples (`Membership`, :6, :284-305).
5. **Cases by shape.** `row.target()` match: `Resolved`, `Intrinsic(CoreIntegerOperator)`, `Intrinsic(other)`, `LateBound` with `CheckedOperator` answered only if completed (:1043-1105), else error "remains unresolved as ..." with a long diagnostic (:1084-1090). `runtime_call` conflicts: refuses a computed call that also carries nominal or static routing fields (:526-557). Build designation: preparation phase must carry no inputs (:700-707).
6. **Same job elsewhere.** `declaration_custody` here is a thin wrapper of `lowered_source_publication::selection_questions::declaration_custody`; `proof_scope.rs:705` has its own `declaration_custody` fn of the same name and 7 `selection_questions` uses elsewhere (grepped, not read).
7. **Unused/one-caller.** grepped: `capture_with_bodies` production calls: `lib.rs:1032`, `read/calling.rs:1293`; `capture` (test-only) used in `read/calling.rs:1564, 1579` (these are inside a test module or not; not verified); `declaration_custody`: `proof_scope.rs:409`, `read/provider_selections.rs:234` and the underlying one in `07_.../source_selection_publication.rs:1742`; `bind_invocation_declarations`: `lib.rs:1016, 1247`.
8. **Stated limits.** Documentation of vacuous disjuncts: "Their operators owe no selection" (:30-31).
9. **Fail-open.** (a) `expression()` returns `false` (skip subtree) for any expression in `completed_operators.is_vacuous(owner, expr)` (:892-894), stated at :890-891 and :27-31 (the reader decided the disjunct false and never read it); a wrong vacuous mark would hide an operator question; the only producer is `skip_vacuous_disjuncts` (`source_operators.rs:56-58`) fed by `guarded_false`, read. (b) `callable_signature_contents` returns `false` (skip) for a signature that is a template requirement or a closed signature template with no admitted view (:842-855), stated at :839-841. (c) `type_reference`/`proof_fact`/`expression` return `false` for already seen keys (dedupe, :977, :985, :887). (d) `machine_body` returns `bodies.contains(machine.symbol)` (:809), so a machine not in the body roster is not walked; traced to `capture_with_bodies` precondition `body_roots ⊆ admitted` (:1245).

Tests (1360-2235, read): operator answers require exact owner, expression, occurrence; cast domain dependencies; computed call retention; product type inputs.

---

## 28. source_roster.rs (43 lines)

1. **Purpose.** The product-path entry: lowers the exact Build-selected roster and binds it into a `LoweredSourcePublication` that retains the roster.
2. **Shape.** `lower_roster(roster, typed)` (:14), re-exports `SourceCallingContext`, `SourceCallingInputs`, `SourceCompilationRoster`, `SourceNormalizationInputs`, `SourceProgramEntryInput` (:6-9).
3. **Inputs/outputs.** Builds a `Compilation` from roster fields with `purpose: DependencyScope::Product`, `publication: Portable` (:18, :27), calls `prepare_compilation` then `compilation::bind_source(typed, publication, prepared, Some(roster))`.
4. **How it decides.** none (hard-codes `Product` and `Portable`).
5. **Cases by shape.** `std::slice::from_ref(&roster.subject)`: exactly one subject (:26).
6. **Same job elsewhere.** `lower_for_publication` in `compilation.rs:45` (outside unit) does the non-roster form.
7. **Unused/one-caller.** grepped `lower_roster`: 1 caller, `driver/judge_lowered_source.rs:116` (and an import at :14).
8. **Stated limits.** none.
9. **Fail-open.** none seen.

---

## 29. static_contracts.rs (415 lines)

1. **Purpose.** Data and publication of "static schemas": the retained requirement schemas, selected conformance questions, and record-binding rows, plus their conversion from source ids to Terminal Psi service ids.
2. **Shape.** `SourceServiceCeiling {Open, Closed}` (:21), `StaticSourceEnvelope` (:26) with `publish` (:34) and `service_roots` (:67), `SourceSchemaRequirement` (:82), `SourceSchema` (:91), `SourceRecordBinding` (:97), `SourceImplementation` (:103), `SourceConformance` (:108), `StaticContracts` (:125) with `conformance_questions` (:140), `record_binding(_mut)` (:174, :179), `insert_record_binding` (:187), `service_roots` (:219), `publish` (:229); `FormedConformance` (:134).
3. **Inputs/outputs.** `publish` reads signatures/outcomes/records/callables/boundaries/services/qualifications and writes `module.identities` (taken and rebuilt, :243, :412); returns `(Vec<StaticSchemaQuestion>, Vec<StaticConformanceQuestion>)`.
4. **How it decides.** Rows are sorted vecs keyed on `(arena_index, generation)` with `binary_search` (:146, :160-163, :194-202). Native conformance implementation resolved by `native.iter().position(|c| c.source == source)` (linear scan, :353-356).
5. **Cases by shape.** `publish` needs `source_reader` to be `Some` for requirement instances and providers (:289-292, :379-384).
6. **Same job elsewhere.** `service_roots` duplicates `read/services.rs::source_roots` (:1387) (read both).
7. **Unused/one-caller.** `conformance_questions`: not grepped; `StaticContracts::publish`: not grepped.
8. **Stated limits.** none.
9. **Fail-open.** none seen; missing rows return refusals.

---

## 30. suspension.rs (65 lines)

1. **Purpose.** Marks every call operation whose callee may suspend with a crossing id, then publishes one site and one plan per crossing from the finished module.
2. **Shape.** `publish_crossings` (:23).
3. **Inputs/outputs.** Reads `terminal_semantics::suspension_frontier::{call_may_suspend, MachineFrontiers, call_site}`. Mutates `operation.suspension_crossing`, `module.execution.suspension_call_plans/sites/plan_count`.
4. **How it decides.** `call_may_suspend(module, operation) == Some(true)` (:28). Terminator calls are never given a crossing (doc :12-14).
5. **Cases by shape.** Operation calls only.
6. **Same job elsewhere.** none.
7. **Unused/one-caller.** 1 caller, `lib.rs:1373` (grepped).
8. **Stated limits.** "A terminator that calls is given none: the checker refuses a possibly suspending call there by name" (:12-14).
9. **Fail-open.** `call_may_suspend == Some(true)` only: `None` (unknown) is treated as not suspending and gets no crossing (:28). Open: no comment gives the reason; a wrong program is one whose callee suspension is unknown to `call_may_suspend`. Traced no further (the function is in `terminal_semantics`, not read).

---

## Coverage

Read in full, every line, in order (31 ranges, 21,846 lines):
`read/record_parameters.rs` 1-1163; `read/references.rs` 1-956; `read/reflection.rs` 1-630; `read/regions.rs` 1-880; `read/regions/result_lifetimes.rs` 1-539; `read/rejections.rs` 1-703; `read/review_requirements.rs` 1-259; `read/services.rs` 1-1720; `read/source_operators.rs` 1-772; `read/source_records.rs` 1-1850; `read/source_records/layouts.rs` 1-490; `read/source_records/opaque.rs` 1-298; `read/source_records/opaque/tests.rs` 1-285; `read/source_views.rs` 1-53; `read/statements.rs` 1-753; `read/static_contexts.rs` 1-214; `read/storage_metadata.rs` 1-264; `read/storage_path.rs` 1-245; `read/subjects.rs` 1-1230; `read/traits.rs` 1-1276; `read/transitions.rs` 1-618; `read/type_equations.rs` 1-198; `read/type_parameters.rs` 1-199; `read/types.rs` 1-1856; `reference_ends.rs` 1-287; `reference_results.rs` 1-1224; `selection_layout_query_tests.rs` 1-126; `selection_questions.rs` 1-2235; `source_roster.rs` 1-43; `static_contracts.rs` 1-415; `suspension.rs` 1-65.
Skimmed: none. Not read: none. Test modules were read but are summarised in one or two sentences, not itemised.

Greps run (all over `source-rust/`): `declared_result_lifetimes|declared_reference_outputs`; `capture_with_bodies|selection_questions::capture|declaration_custody\(|bind_invocation_declarations`; `lower_roster|publish_crossings|reference_ends::install|reference_results::publish|build_entry_context|static_reach_contexts|generic_type_alias|generic_trait\(|generic_data\(`; `\.same_signature\(|\.declaration_query\(|\.review_requirements\(|...` excluding `tests.rs`; `\.mistyped\(` in count mode (11 across 7 files).

## Five things a maintainer most needs to know about this unit

1. **Two refusal levels.** `refuse` means "not read yet"; `reject` means "the spec defines this as an error for every program of that shape". `read/rejections.rs:1-49,91-217` is the single place for receiving-type mismatches; everything else it cannot judge stays a refusal. Most of the unit's "cases by shape" lists are refusals, not rejections.
2. **Capture can only fail closed on operators.** `read/source_operators.rs` records builtin answers only for operations the reader really formed. A bare operation with no builtin meaning is skipped silently (`:538-548`), and `selection_questions.rs:1043-1092` later fails with "remains unresolved as CheckedOperator" unless the exact (owner, expression, occurrence) was recorded. So the pair of files together is the check; neither alone accepts or rejects.
3. **Operational envelopes are a fixed point inside `read/services.rs:186-645`.** It keeps authored ceilings (even when a call exceeds them) and infers omitted axes, tracked by `KnownAxes`. Only machines whose axes are all known are bound into `callables` (`:636-641`); termination for private bodies is assumed from the ranked-cycle structure (`:536-542`).
4. **Reference and lifetime handling is split across three layers in this unit**: region algebra (`regions.rs`), declared result lifetimes (`regions/result_lifetimes.rs`, used at 4 production sites), and loan/lender publication after lowering (`reference_results.rs`, `reference_ends.rs`). Each repeats the `ReferenceAccess`/`StructuralAccess` mapping instead of sharing one function.
5. **`read/subjects.rs` is where "unjudged declaration" is enforced**: measures, operators, propositions and wire schemas that are subject declarations are refused (`:633-660`); names-only data and tag-only domains are passed over by design (`:1177-1223`). Generic templates are accepted only under the narrow lists at `:706-885`, `traits.rs:618-687`.

## What I am unsure about

- Whether `capture` (`selection_questions.rs:1199`, `#[cfg(test)]`) is also what `read/calling.rs:1564,1579` calls (those lines may be inside a test module; I did not read them).
- The behaviour of `terminal_semantics::suspension_frontier::call_may_suspend` for `None` (`suspension.rs:28`); I did not read it.
- Whether `read/arrays.rs` and `read/bytes.rs` have other callers of `source_views::source_view` (only one found in this unit).
- Whether the parallel-row pushes in `record_parameters.rs:195-206` match those of `Program::requirement` exactly; I read only the first.
- Counts for most `pub(super)` functions were not grepped; they are listed as "not grepped" where that applies.
