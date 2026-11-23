# Record: unit_075 (areas 05-source-representations, 06-source-driver-build, 15-package-manager)

Checkout `4f3c255d0559`. Paths are relative to the checkout. `TR/` below means
`source-rust/subsystems/compiler/source-to-psi/representations/typed-trees/type_records/`.
All 13 files were read in full. Citation forms: "read" = read at that line; "grepped" = a command I ran
(Grep tool, count or content mode, over `source-rust/`).

Seven of the thirteen files belong to the `typed-trees` crate (area 05). Two are the source-to-psi driver crate's
root and test support (area 06). The last four are tests of the driver or the `compiler` crate. Nothing in this unit
sits in `15-package-manager`; the unit label covers it only because the tests reach the package-facing API
(`PackageKeyIdentity`, `ProductDeclarationScope`).

---

## 1. `TR/data_applications.rs` (470 lines)

**Purpose.** Forms, for each applied data type such as `Chain<u8>`, a "view": a copy of the data declaration whose
member types have the type and const arguments substituted in. Typing makes the first views; specialization and
compile-time evaluation call the same operation (module comment, `:1-4`).

**Shape.**
- `prepare_from(program, first)` (`:14`): finds each `Generic` type reference from row `first` onward whose base is a
  unique data definition (`:18-27`), then forms its view through `ApplicationPreparation`.
- `prepare(program, reference)` (`:39`): the same for one reference; returns `Ok(true)` when the types are closed.
- `settle_const_argument(program, argument, spelling)` (`:58`): once an evaluator has computed `Buffer<capacity()>`'s
  argument, rewrites that `ConstExpression` node in place to a `Named` node holding the canonical spelling (`:103-109`),
  turns dependent `FixedArray` lengths from `Expression` to `Literal` (`:110-120`), then discards and re-forms every open
  view that mentions the argument (`:123-128`).
- `mentions` (`:135`): recursive search for an argument handle inside a type; depth capped at 64 (`:153`).
- `ApplicationPreparation` (`:203`): fields `source: &mut TypedTrees`, `active_references`, `active_declarations`.
  Methods `reference` (`:218`), `form` (`:244`), `application` (`:297`).

**Inputs / outputs.** Reads `type_reference_table`, `data_definition` rows, symbols, `data_type_parameters`.
Mutates the program: appends rows through `retain_data_application_definition` (`:447`), `tables.data_members`
and `tables.data_payload_fields` (`:437,441`), and inserts generated `Named` rows for canonical const values (`:383-389`).

**How it decides.**
- A reference already on the active stack is accepted only if it has an existing view (`:233-235`, comment explains the
  `data Seq<T> { case Cons(head: T, tail: Seq<T>); }` self-application case); otherwise refused as cyclic (`:236`).
  Keyed on type-reference handle identity.
- A declaration already on `active_declarations` gets a shallow view and returns `closed` without expanding
  its children (`:449-456`). Keyed on owner symbol handle.
- A view that already exists for `reference` is reused if `retained.symbol == owner` and
  `generic_instance == Some(reference)`; mismatch is refused (`:361-366`).
- "Closed" means every `Type` argument is closed and every `Const` argument has a static literal or canonical value
  (`:393-408`), and every child passes `index_argument_is_closed` (`:444-446`).
- Const arguments are canonicalized by `value_i64`/`value_u64` and replaced with a generated name of the decimal text (`:371-391`),
  so the key is a spelling of the number.

**Cases by shape.**
- `application` handles only data whose parameters are all `Type` or `Const` and has no lifetime arguments:
  `if lifetimes != 0 || parameters.iter().any(|p| !matches!(p.kind, Type | Const))` returns `Ok(false)` (`:321-330`).
  Value, Machine, Implementation and Proposition parameters get no view and report "not closed".
- Non-data bases return `Ok(false)` with the comment "Applications of machines and other namespaces retain their own source
  records" (`:304-308`).
- `form` handles each `TypeReferenceNode` variant; `ConstExpression` returns `Ok(false)` (`:293`).
- `settle_const_argument` rewrites only `FixedArray` lengths that are the `Expression` of this argument, and applications
  that mention it (`:73-92`).

**Same job elsewhere.** The substitution is also done by `TypeReferenceTable::instantiate` (`TR/type_system.rs:1918`), which
this file calls (`:414,422,431`). `closed_data_definition` in `TR/type_identity.rs:9381` finds an instance by scanning
`data_definitions()` and comparing identities; it is a second way to find a specialized data definition (read).

**Unused / one-caller.** `settle_const_argument`: grepped 7 occurrences over 4 files (definition, a doc mention in
`type_system.rs`, 2 in `driver/compile_time_invocations/constants/arguments.rs`, 3 in pipeline 03 tests). `prepare`: grepped
callers in driver `calls/arguments.rs:144`, pipeline 03 (`call_and_product_selections.rs:2166`, `statement_lowering.rs:732`),
pipeline 04 tests, pipeline 06 tests, `static_operand_binding/mod.rs:596`. `prepare_from`: pipeline 03 `lowerer.rs:1915`,
`typing_queries.rs:2140`; pipeline 04 `specialization.rs:206,1830`.

**Stated limits.** "Repeated declarations through their member definitions may describe recursive storage. Retain this exact shallow view;
do not expand an unbounded family or grant it a finite layout" (`:450-453`). "Views do not publish declarations or establish their
generic premises" (`:13`).

**Fail-open risks.** none seen. Unknown shapes return `Ok(false)` (not closed), which is the refusing direction. One spot to
note: `application` returns `Ok(false)` for a non-data owner (`:304-308`) rather than an error; a caller that treats
`Ok(false)` as "no view needed" would skip it. Classification: stated (the comment says these retain their own records).

---

## 2. `TR/data_applications/tests.rs` (134 lines)

**Purpose.** Two unit tests of the above: a self-applied data type keeps one view (`:95`), and a type that contains itself
with no declaration between is refused with the exact text "data application has a cyclic type-reference graph" (`:117-133`).
Builds a hand-made `TypedTrees` for `data Chain<T> { case Link(head: T, tail: Chain<T>); }` (`:9-92`).
Positive/negative: the second test makes a `Slice` that points at itself (`:119-129`).

---

## 3. `TR/mod.rs` (8 lines)

Declares the five modules `data_applications`, `opaque_representations`, `type_identity`, `type_system` (all `pub mod`),
`source_type_applications` (`pub(crate)`), and `boundary_values` (`:3-8`). `boundary_values` is outside this unit; the file
is not in the unit list (not read).

---

## 4. `TR/opaque_representations.rs` (224 lines)

**Purpose.** Holds the record of Build's admitted carrier for an opaque declaration: which opaque symbol, which carrier,
which closed conformance application, lifecycle and copy disposition, and a 32-byte commitment over those (`:15-27`).

**Shape.** `OpaqueRepresentationSelection` (`:16`; fields `opaque`, `carrier`, `carrier_type`, `application: ClosedConformanceApplication`,
`lifecycle`, `copy_disposition`, `origin`, `selected_application_commitment`, `selecting_machine`, `source_span`), all private with getters.
`from_validated_application` (`:53`) computes the commitment with `selected_application_commitment(...)` from the `application_identity` crate.
`portable_application` (`:30`) builds `SelectedOpaqueApplication` by calling `typed.normalized_hermetic_symbol_identity` on opaque, carrier and
conformance declaration (`:37-40`). `rederived_selected_application_commitment` (`:120`) recomputes the commitment.

**Inputs / outputs.** Pure record plus re-exports from `application_identity::opaque_representations` (`:7-11`).

**How it decides.** Nothing is accepted or refused here. The name `from_validated_application` says the caller validated; nothing
in this file checks that. Comment at top: "Admission establishes the choice" (`:4`).

**Unused / one-caller.** `rederived_selected_application_commitment`: grepped 3 occurrences, all in this file (definition and the
test at `:210,218`), so it has no non-test caller in `source-rust/` (grepped). `from_validated_application`: used by the tests here;
callers elsewhere not grepped (suspected: Build).

**Stated limits.** none.

**Fail-open risks.** `from_validated_application` takes already-built values and stores them (`:53-82`); a caller passing an unvalidated application
gets a record with a self-consistent commitment. Classification: open (the name asserts validation; I did not trace callers).

---

## 5. `TR/source_type_applications.rs` (528 lines)

**Purpose.** Gives a scope chain of type substitutions (`SourceTypeScope`) the operations needed to read a type "as supplied": follow a
named binder to its actual argument, view a slice or a borrowed slice, and ask for type identities and integer-argument matches under that scope.

**Shape.**
- `SourceSliceApplication` (`:11`): `reference`, `referent`, `slice`, `element` (each a `SourceTypeApplication`), `access: Option<ReferenceAccess>`.
- `impl SourceTypeScope` (`:19`): `with_substitutions` (`:20`, flattens the parent chain into one substitution list plus `starts` offsets and a `rigid` list),
  `supplied_type_reference` (`:52`), `slice_application` (`:92`), `type_identity` (`:142`), `index_identity` (`:158`), `integer_argument_matches` (`:173`),
  `declared_carrier_identity` (`:191`), `carrier_identity_in_region_scope` (`:207`), `type_identity_in_region_scope` (`:227`), `region_free_type_identity` (`:245`),
  `admits_argument` (`:264`).
- `SourceTypeApplication::type_identity` (`:278`).

**Inputs / outputs.** Reads `TypedTrees` and delegates to `TypedTrees::scoped_*` identity functions in `TR/type_identity.rs`
(`:148,164,181,197,214,234,251,266`). Produces `NormalizedTypeIdentity` / `Option<...>` / `bool`.

**How it decides.**
- `supplied_type_reference` loops: if the reference is a `Named` and some frame has a substitution keyed on that symbol, replace the reference
  and move to that frame's parent (`:71-88`). Keyed on symbol handle. Returns `Err("supplied type lost its retained type reference")` for a stale handle (`:64`).
- `slice_application` accepts a `Slice` or a `Reference` to one, peels `Constrained` wrappers with a counter bounded by the type-reference count
  (`:110-123`, error "supplied slice has a cyclic qualification tree"), and returns `Ok(None)` for anything else (`:107,126`).
- `integer_argument_matches` is `admits_argument(...) && scoped_scalar_argument_matches(...)` (`:179-188`).

**Cases by shape.** `slice_application` handles only `Slice` and `Reference{referee: Slice}` (`:99-108`); a reference to a reference to a slice returns `None`
(test asserts this at `:396-402`).

**Same job elsewhere.** `with_substitutions` + `scoped_*` is a thin adapter; the same identity entry points are also reached with explicit
`substitutions` slices through `TypeIdentityRequest` (`TR/type_identity.rs:9310`). `applied_*` functions in `type_identity.rs` use `AppliedTypeIdentityScope` instead of
`SourceTypeScope` for the same job of "identity under a supplied scope" (read; two representations of a scope).

**Unused / one-caller.** Not grepped individually (`SourceTypeScope` is defined outside this unit). Suspected callers in pipeline 03/05.

**Stated limits.** "Shape inspection grants neither backing nor access permission" (`:9`). "Sibling bindings in the same tuple cannot rewrite one another's actuals" (`:50`).

**Fail-open risks.** `supplied_type_reference` returns the unchanged application when no frame binds the name (`:84-86`); this is "no substitution", which is the correct meaning for a free name. Classification: traced (the function's contract is substitution, not validation).

Tests (`:283-527`): `shared_source_frames_preserve_simultaneous_substitution` and `constant_arguments_keep_their_carrier_and_simultaneous_scope`; both build symbol tables by hand.
Negative cases are inside the tests: a foreign-carrier scope fails `integer_argument_matches` (`:517-526`), and the invalid handle is refused (`:403-407`).

---

## 6. `TR/type_identity.rs` (11,561 lines)

**Purpose.** Computes canonical, binder-aware text identities for types, index (const) expressions, constraints, lifetimes, callable signatures, specialized
machine applications and declarations, and interns them in `TypedTrees`. The file is one flat file holding about 15 nested `mod` blocks, then about 3,000 lines of
`TypedTrees` methods and three groups of unit tests. Header comment (`:1-5`).

Rough layout (all read):

| Lines | Contents |
| --- | --- |
| 1-15 | `builtin_type_name`, `builtin_type_identity` (forward to `application_identity`) |
| 17-933 | `mod callable_identity`: `CallableIdentityTable` (interner with 5 side maps), requirement-schema bytes, signature-type normalization, tests |
| 934-1211 | `mod constraint_identity`: constraint and array-length normalization |
| 1212-1214 | `mod coordinates`: re-export of `CoordinateTag` |
| 1215-1320 | `mod derived_declarations::nominal_name` (placed/derived view names) |
| 1321-3675 | `mod identity_context`: `TypeIdentityContext`, `SignatureElision`, `normalize_type_identity`, `normalize_index_expression`, `match_dispatch`, tests |
| 3676-3912 | `mod lifetime_parameters` |
| 3913-4452 | `mod normalized_identities`: `TypeIdentityId`, `TypeIdentityTable`, canonical renumbering, tests |
| 4453-4480 | `mod open_index_identity` (operator to `IndexForm` table) |
| 4481-6260 | `mod package_policy_reading_tests` (test-only fixture, witnesses, read-back tests) |
| 6261-7570 | `mod portable_machine_applications` (captures, conformances, specialization identities) |
| 7571-7722 | `mod result_dispatch_terms` |
| 7723-8024 | `mod static_operand_identity` |
| 8025-8084 | `mod storage_identity` |
| 8085-8786 | `mod substitution` (index argument, array length, integer/boolean readback) |
| 8788-9348 | public re-exports, `IndexIdentityArgument`, `AppliedTypeIdentityScope`, `applied_*` / `portable_*` entry points, `TypeIdentityRequest` |
| 9350-10754 | `impl TypedTrees`: data constructor lookups, hermetic symbol identity, domain applications, runtime membership, exact-owner identity, callable identity parts |
| 10756-10788 | `signature_lifetime_binders`, `callable_type_binders` |
| 10790-11561 | tests: closed requirement declarations, nested callback regions, declared array extents |

### 6.1 `callable_identity` (`:17-933`)

**Shape.** `CallableIdentityId`/`CallablePathId`/`CallableParametersId`/`CallableResultDispatchId` (`u32` handles, `0` is invalid, `:32-71`);
`CallableIdentityTable` (`:159`) with `paths`, `parameters`, `result_dispatches`, `records` and four `SharedValue<HashMap>` lookup maps plus `boundary_operator_slots` (`:158-188`);
`intern` (`:254`) builds text `compound(NamedCallable, [Path, Parameters, ResultDispatch])`; `intern_boundary_operator` (`:283`).
`TypedTrees` methods: `machine_callable_identity` (`:369`), `trait_requirement_callable_identity` (`:374`), `requirement_schema_definition` (`:387`),
`requirement_definition_types` (`:485`), `machine_definition_types` (`:549`), `signature_types_in_context` (`:610`), `operator_callable_identity` (`:676`),
`boundary_operator_requirement_callable_identity` (`:683`).

**How it decides.** Interned by `(path, parameters, result_dispatch)` ids, i.e. by the text of the identity (`:259`). Boundary operators are interned by
`(path, Vec<TypeIdentityId>, TypeIdentityId)` (`:288`). Both keys are text/identity values, not symbol handles.
`requirement_schema_definition` writes a binary schema (`omega.requirement-schema.v5\0`, `:401`) of the signature text, lifetime count, per-parameter kind byte
(1 type, 2 const, 3 value), multiplicity byte, and contract text.

**Cases by shape.** `requirement_schema_definition` accepts only `Type`, `Const`, `Value` parameter kinds. `Machine`, `Implementation`, `Proposition` return
`Err("requirement schema needs its complete higher-order parameter contract")` (`:441-446`). Absent schema parameter span, empty signature text,
stale carrier all refuse (`:394-396,398-400,424`).

**Same job elsewhere.** `callable_type_binders`/`signature_lifetime_binders` (`:10756-10788`) are used by both callable identity here and `machine_entry_result_type_identity`
(`:10577`). `runtime_callable_identity::normalize_contract` (another file, grepped call at `:3238,3458`) is the sibling for callable *types*.

**Unused / one-caller.** `boundary_operator_requirement_callable_identity`: not grepped (suspected callers in `definitions/operator_declarations.rs`).

**Fail-open risks.**
- `boundary_operator_requirement_callable_identity` returns `Default::default()` (an invalid id) when the operator is not a boundary, and `unwrap_or_default()` when parts cannot be built (`:687-698`). A caller that does not check `is_valid()` receives the "no identity" id. Classification: open (not traced).
- `intern_callable_identity` maps a `None` parts to the default id (`:738`); `machine_callable_identity` therefore returns an invalid id for a machine with no states (`machine_callable_identity_parts` returns `None` at `:10542` via `?`). The test at `:853` asserts `is_valid()` only for a machine that has a state. Classification: open.
- `callable_identity_text` / `callable_parameters_text` / `callable_result_dispatch_text` return `unwrap_or_default()` strings (`:765-809`): an unknown id reads as the empty string. Classification: open.

### 6.2 `constraint_identity` (`:934-1211`)

**Purpose.** Turn the constraints on a `Constrained` type into sorted, deduplicated strings, and render array lengths.

**How it decides.** `NormalizedConstraint` has four variants `Arithmetic`, `CompilerDomain`, `DeclaredDomain`, `Named` ordered by derive (`:1107-1113`), so output order is by variant then by text.
`declared_domain_identity` prefers `program.semantic_domains.name(domain.semantic_id)`, then the display path of `domain.symbol`, then the written alias (`:1094-1104`); the comment at `:1099-1100` says the alias is diagnostic only.
So a domain is identified by semantic id first, symbol second, and finally by source spelling `domain.name`.
`normalize_array_length` (`:1132`): `Expression` needs a `ConstExpression` node else yields `InvalidExpression` text "missing constant recipe" (`:1139-1146`); `ConstCall` uses `context.name(program, *machine, name)` and so falls back to the written name if the symbol is invalid (`:1155-1158`).

**Cases by shape.** `DomainConstraintSubject` has three arms: `Declared`, `Value(_)`, `OmegaLayout{..}` (`:988-997`); `ValueDomain` has only `Finite` and `OmegaLayoutGrammar` only `Derived` (match at `:1061-1064,1073-1075`).

**Fail-open risks.** `FixedArrayLength::ConstParameter` unbound falls to `unbound_array_length` which yields a `ConstParameter` atom of the name (`:1151-1153`, see 6.14); this is a spelled-name identity for an unbound parameter. Classification: traced to `substitution::unbound_array_length` (`:8591`), which rejects a valid non-value symbol and spells the name otherwise.

### 6.3 `derived_declarations::nominal_name` (`:1215-1320`)

**Purpose.** For a symbol that is a derived "placed" view data type, one of its field accessors, or an accessor's machine/state, produce a derived name from the origin application, policy and schema.

**How it decides.** Loops over `program.access_view_plans` and matches `field.accessor_data_symbol == symbol` or target machine/state symbol (`:1238-1245`). Keyed on symbol handles. Then authenticates the join: the origin must be a `Generic` of `view.template_symbol` with exactly `[policy, schema]` operands matching `view.policy_symbol`/`schema_symbol` (`:1251-1272`). With `hermetic=true` each of template/policy/schema must have a package identity or a toolchain origin (`:1273-1285`).
`Ok(None)` if no view mentions the symbol (`:1318`).

**Fail-open risks.** none seen; each mismatch returns `Err` with a reason. The `Ok(None)` arm means "not a derived declaration" and is the intended fallthrough.
Note a linear scan of `access_view_plans` per call (`:1237`).

### 6.4 `identity_context` (`:1321-3675`)

**Purpose.** The single recursive normalizer. `TypeIdentityContext` (`:2014-2067`) carries about 30 optional inputs (binders, substitutions and their scope starts, application scopes, lifetime binders, signature elision state, qualification, exact toolchain sources, three `Cell<bool>` "invalid" flags, etc.).

**Shape.**
- `normalize_type_identity` (`:2960`) returns `TypeIdentityId`; `normalize_type_reference` (`:2944`) returns its text.
- `normalize_index_expression` (`:3283`) for const expressions.
- `Composite` (`:1348`) pairs a tag with `IdentityPart`s, interned or read as text.
- `SignatureElision` (`:2075`) with `Cell`s `next`, `minted`, `position` and state enum `ElidedPosition` {Fresh, FirstResult, Only, Ambiguous} (`:2082-2088`).
- `SignatureRegion` {Bound, Captured, Argument} (`:1993`), `SubstitutionSemantics` {Transitive, Simultaneous} (`:2147`), `TypeIdentityQualification` {Ordinary, PackageQualified} (`:2154`), `PackageQualifiedNominalOwner` (`:2161`).
- `match_dispatch::normalize_match` / `normalize_saved_subject` (`:1381-1565`).

**How it decides.**
- Order of cases in `normalize_type_identity` (`:2960-3281`): applied-argument by symbol (`:2997`), index substitution by symbol (`:3006`), data definition with `generic_instance` (`:3026`), substitution list by symbol, searched from the end (`:3035`), static type refinement (`:3068`), then the node match.
  All keyed on `SymbolHandle`.
- `context.name` (`:2494`): when the symbol is invalid and no `::` in the fallback it searches `binders` by *symbol display name equal to the fallback string* (`:2512-2521`): `program.symbols.name(*candidate) == fallback`. This is a source-spelling lookup used when the symbol handle is absent. If two binders with that name map to different replacements it rejects (`:2517-2519`).
- `qualify_non_binder_name` (`:2532`): chooses `PackageQualifiedNominalOwner` by `symbol_package_identity`, else toolchain source digest (by comparing the declaring file's `SourceId` against `exact_toolchain_sources`, `:2574-2579`), else `Toolchain`/`Unresolved`. In `Ordinary` qualification, a builtin or a symbol with no package identity and no toolchain origin is named by its display path `path` (`:2553-2562`), i.e. by spelling.
- `lifetime` (`:2255`): resolves a lifetime *name string* by walking owners and comparing `parameter.as_str() == name` (`:2268-2272`); fallback via `applied_lifetime_parameter_identity_with_supplier`. Source spelling of a lifetime name is the key there.
- `encode_signature_region` (`:2405`): when neither package-qualified nor structural, the region owner is printed as `"{arena_index}:{generation}"` of the declaration symbol (`:2448-2461`).
- Reference lifetime: with an elided lifetime in a package-qualified context, sets `encountered_elided_regions` and emits `RegionForm::Elided` (`:3110-3116`).

**Cases by shape.**
- `normalize_index_expression` handles every `ExpressionNode` variant but refuses (calls `substitution::rejected`) for: `Block` (`:3412-3413`, comment "A block value is no index term"), `Unary::Dereference` (`:3386-3388`), `Binary` with `policy != Exact` (`:3372-3374`, "Until that complete operation is represented, refuse an exact identity"), `Member` with `MemberSelection::Field(invalid)`/`Unresolved` in structural/schema contexts (`:3566-3568`), `MatchPattern::Guard` (`:1447-1449`, "A match without a subject is no index term").
- `SignatureElision::enter_result` (`:2107`): result elision is `FirstResult` if no lifetimes, `Only(n)` if one, `Ambiguous` if more. The comment cites lifetimes.md "Returned views" and says stage 05's `Elision::Inputs`/`Elision::Result` read the same rule (`:2069-2074`).
- `match_dispatch::normalize_match` accepts a destructuring pattern only if the match has a capture (`:1419-1433`).

**Same job elsewhere.** The elision rule here duplicates stage 05's: the comment says it explicitly (`:2072-2074`); I did not read stage 05 (suspected duplicate).
`normalize_const_or_nominal_name` (`:3650`) and `substitution::index_name` (`:8364`) both decode canonical const atoms and decimal integers from a *spelling* (`from_atom`, `parse::<i128>`, `:3658-3671`; `:8370-8415`).

**Unused / one-caller.** `Composite::text` used at `:2921`. Others not grepped.

**Stated limits.** "A raw non-Exact operation needs its selected carrier as well as its policy. Until that complete operation is represented, refuse an exact identity" (`:3369-3371`). "Existing checked identities point this observation at their error flag" (`:2041`).

**Fail-open risks.**
- `normalize_type_reference` calls `.expect("a normalized identity was minted by the program's table")` (`:2951-2953`); a panic, not an accept.
- `TypeReferenceNode::Named` with an invalid symbol and a spelling that parses as an `i128` or a canonical const atom is read as a constant (`:3657-3672`), and otherwise named by its spelling via `context.name` (`:3673`). In an `Ordinary`, non-structural context with no `invalid_application` flag, an unresolved name is accepted and identified by its text. Classification: traced to callers using `TypeIdentityRequest::ordinary` (`:9319`), which sets no invalid flags (`type_identity_with_substitution_semantics`, `:10428-10449`): so plain `normalized_type_identity` never reports an unresolved name; only the `applied_*`/exact entry points set `invalid_application`.
- `lifetime()` returns `bare(RegionForm::Unresolved)` and sets the invalid flag only if `invalid_application` is `Some` (`:2311-2314`). With no flag supplied the unresolved region becomes text silently. Classification: traced (same as above).
- `match_dispatch::unavailable` sets three flags and returns `UnboundMatchSubject` text (`:1394-1405`).

### 6.5 `lifetime_parameters` (`:3676-3912`)

**Shape.** `LifetimeParameterIdentity { declaration: SymbolHandle, ordinal: usize }` (`:3684`); `portable_lifetime_parameter_identity(_in_context)` (`:3689,3703`); `applied_lifetime_parameter_identity(_with_supplier)` (`:3749,3765`); `apply_resolved_lifetime_parameter_identity` (`:3804`); `lifetime_parameter_identity(program, owner, name)` (`:3833`); `declaration_lifetime_parameters` (`:3863`).

**How it decides.** `lifetime_parameter_identity` walks owners outward and matches `parameter.as_str() == name` (`:3841-3845`): a *name string compare* inside a declaration, yielding an ordinal. `portable_lifetime_parameter_identity_in_context` chooses a `kind` string by probing in this order: machine, trait requirement, conformance, trait, type alias, data (`:3710-3734`); the kind text and the declaration's callable identity/display path are embedded in the identity.
`declaration_lifetime_parameters` tries machine, alias, conformance, trait, data, trait-machine-signature, runtime callable contract, retained static machine contract in order (`:3867-3910`).

**Cases by shape.** The runtime-callable branch accepts an owner only if *all* contracts whose `signature.symbol == owner` have equal lifetime rosters (`:3898-3905`); otherwise `None`.

**Fail-open risks.** none seen (returns `None` on no match).

### 6.6 `normalized_identities` (`:3913-4452`)

**Shape.** `TypeIdentityId(u32)`; `TypeIdentityTable { texts: OwnedTextInterner, composites: RwLock<Arc<CompositeIndex>> }` (`:3965`); `IdentityPart::{Identity, Text}` (`:3990`); `intern`, `text`, `by_text`, `intern_composite` (`:4096`), `composite_text` (`:4127`), `renumber_canonically` (`:4082`); `NormalizedResultDispatchSet`, `NormalizedDomainTerm` (`:4420,4447`).

**How it decides.** Identity is the *text*: two identities are equal iff the interned strings are equal. The composite index is a cache keyed by `(tag, parts)`; the comment says "Only the texts are the table's record: the index is rebuilt as identities are asked for again" (`:3978-3979`). `PartialEq for TypeIdentityTable` compares only `texts` (`:4010-4014`).
Record writing renumbers identities into sorted-text order so that equal programs write equal bytes (`:4027-4035`); a second table under another's numbering and an identity minted after numbering both refuse (tests `:4241-4265`).
`NormalizedResultDispatchSet::identity` formats `"arithmetic:{name}&compiler:{..}&declared:{..}"` (`:4433-4443`), a plain string concatenation.

**Unused / one-caller.** `TypeIdentityTable::storage_index` (`:3949`): not grepped.

**Fail-open risks.** `TypeIdentityTable::by_text` returns the default (invalid) id when the text is absent (`:4171-4176`). Classification: open (callers not traced).

### 6.7 `open_index_identity` (`:4453-4480`)

`index_binary_operator_name` maps each of 19 `BinaryOperator` variants to an `IndexForm`; exhaustive match, no wildcard.

### 6.8 `package_policy_reading_tests` (`:4481-6260`)

Test-only (`#[cfg(test)]`). `fixture::Fixture` (`:4543`) builds a `TypedTrees` with four user packages (digests `[1;32]..[4;32]`) plus one toolchain source, 23 declarations (table `DECLARATIONS`, `:4512-4536`), a `Container<T, const N, value V>` data, a placed-view plan, a callback contract and a `Selected` machine. `kinds` (`:5181`) is a `node_kinds!` macro enumerating every `TypeReferenceNode`, `TypeConstraintNode`, domain, length, expression, operator, pattern and action variant. `witnesses` (`:5300`) builds one type per form with its expected package-owner set. Tests (`:5972-6259`) check that every rendered identity reads back through `application_identity::visit_type_identity_package_owners` with exactly the expected owners, that repeated requests mint no new text or composite, that callable/result-owner tampering is refused, and that a stray marker is refused.
Negative twins: `an_unflagged_marker_is_refused_with_its_reason` (`:6247`); foreign owner / invalid owner for result (`:6119-6135`).

### 6.9 `portable_machine_applications` (`:6261-7570`)

**Purpose.** Gives a specialized or captured machine a portable identity built from its template identity plus its argument tuple, checked against retained binding accounts.

**Shape.** `PortableMachineApplications { active: RefCell<Vec<SymbolHandle>>, error: RefCell<Option<String>> }` (`:6963`); methods `selection` (`:6991`), `nominal_name` (`:7264`), `identity` (`:7281`), `origin` (`:7350`), `application` (`:7379`), `bound_application` (`:7407`), `record_value` (`:6969`), `capture` (`:6273`), `conformances::project` (`:6425`), `validate_selected_scope` (`:6653`), `static_binding_operand` (`:7528`). Public wrappers `portable_machine_application_identity` (`:6773`), `portable_machine_selection_identity` (`:6803`), `portable_operation_record_value` (`:6848`).

**How it decides.**
- Cycle guard: `active` contains the machine or record symbol, else error "...cyclic static argument identity" (`:6975,7299`). Keyed on `SymbolHandle`.
- Parameters are matched to bindings by `binding.parameter == parameter.symbol` with exactly-one rule (`:7458-7466,6322-6331,7175-7183`).
- Binding kind is checked against parameter kind in `static_binding_operand` (`:7532-7568`): pairs (Type, Type), (Const, Const), (Value, Const), (Value, RuntimeValue with no subject and no expression), (Machine, Machine), (Implementation, Conformance); any other pair is `Err`.
- `selection` compares the supplied source tuple to the generated receipt by identity text equality (`:7253-7259`).
- `conformances::project` checks the selected rows against the declaration rows field by field (`:6437-6451`) and compares the declared and supplied static arguments by identity text (`:6485-6496`).
- `capture` checks bindings, lifetimes, state correspondence (`:6316-6395`).

**Cases by shape.**
- `selection` refuses unless the selected symbol is a machine, or the *first* entry state of one: "selected callable has no exact initial entry" (`:7008-7015`).
- Captured callable: "captured callable needs its exact own specialization account" if there is an own tuple (`:7075-7098`).
- Const literal or `evidence_projection` on the selected argument: refused (`:6998-7003`).
- A monomorphic callable "retains unexpected static bindings" if bindings exist (`:7227-7237`).

**Same job elsewhere.** `capture`'s binding/argument matching loop (`:6321-6346`) and `selection`'s explicit-tuple loop (`:7174-7197`) and `bound_application`'s (`:7457-7493`) repeat the same "find unique binding for parameter, then compare identity text" idiom three times in this file (read). `portable_record_schema_identity_in_context` (`:6898`) repeats the trait-application production in `conformances::project` (`:6505-6543`); its own comment says "The existing schema production, with the current projection kept" (`:6903`).

**Unused / one-caller.** `portable_machine_application_identity`, `portable_machine_selection_identity`: re-exported `pub` (`:8805-8807`); not individually grepped.

**Fail-open risks.**
- `identity` returns `Ok(None)` when the symbol is not a machine or has no instantiation receipt (`:7287-7298`). `nominal_name` converts that to `None`, letting the caller continue with an ordinary name (`:7264-7279`). Wrong program: a specialized machine whose receipt is missing is named like an unspecialized one. Classification: open (callers do `projection.nominal_name(...)` at `:2539` and `:8294`; `machine_instantiation(...)` itself may return `Err`, which is propagated at `:7293-7295`; I did not trace when it returns `Ok(None)` for a specialized machine).
- `PortableMachineApplications::nominal_name` stores the first error in `self.error` and returns an `InvalidDerivedOrigin` atom (`:7270-7277`); the public wrappers then check `projection.error` (`:6791,6822,6889`). The `portable_operation_record_value_in_context` path likewise (`:6889-6891`). Classification: traced.

### 6.10 `result_dispatch_terms` (`:7571-7722`)

`collect_result_dispatch_terms` walks `Reference` and `Constrained` down to the base, collecting arithmetic, compiler and declared-domain terms (`:7582-7635`); every other node kind adds nothing (`:7627-7633`).
`collect_declared_result_dispatch_terms` expands domain aliases with a stack guard against alias cycles (`:7649-7652`, returns silently on a cycle) and **drops** a domain that has a predicate and no semantic roles and is `Unrestricted` (`:7682-7688`) or whose constraint record has a predicate and no roles and no routes (`:7697-7701`).
Cases by shape: only `Reference`/`Constrained` are traversed; arrays, slices and generics contribute no dispatch terms.
**Fail-open risk:** alias cycle returns without a term (`:7649-7651`) and a domain "silently dropped" from the overload identity when it has a predicate and no roles. Wrong program: two overloads that differ only by such a predicate-only domain get the same callable identity. Classification: open (the drop is deliberate per code shape, no comment explains why).

### 6.11 `static_operand_identity` (`:7723-8024`)

`specialization_source_operand` (`:7751`) rebuilds a `StaticMachineArgument` from a `MachineSpecialization` receipt: requires the template's own parameter count to equal the binding owner's count and refuses if `binding_declaration != receipt.template` and the template has lifetime parameters (`:7773-7779`). Runtime values are refused as keys (`:7827-7829`). `specialization_source_identity` (`:7865`), `declared_static_operand_identity` (`:7880`), `static_operand_identity` (`:7904`) and `..._with_simultaneous_substitutions` (`:7911`). Each builds a `TypeIdentityContext` with the three invalid `Cell`s tied together and returns `None` if the cell is set.
Test `receiving_static_indices_do_not_resubstitute_sibling_bindings` (`:7955`).

### 6.12 `storage_identity` (`:8025-8084`)

`independent_value_index` (`:8031`): a `Value` parameter is "independent" if the owner is `CheckedShape` supply mode with no quotient and no field's type mentions the parameter (via `independent_type`, depth cap 128, `:8058`). Used by `generic_reference_identity` when `context.storage_value_indices` is set (`:2784-2790`), replacing such an argument with `Unit` text.
Cases by shape: `independent_type` accepts `Named`, `Reference`, `Slice`, `FixedArray` with `Literal` length, `Generic`, `Unit`; every other node returns `false` (`:8062-8082`).

### 6.13 `substitution` (`:8085-8786`)

**Purpose.** Index arguments (const/value operands) to identity text.

**Shape.** `index_argument` (`:8107`) dispatches on `IndexIdentityArgument` {TypeArgument, Expression, TypeReference, Subject, FieldProjection, StaticArgument}. `index_name` (`:8364`), `selected_index` (`:8434`), `scalar_argument_matches` (`:8481`), `array_length*` (`:8572-8730`), `rejected` (`:8732`), `boolean`/`integer` readbacks (`:8745,8764`).

**How it decides.**
- `selected_index` looks up by symbol: applied scope argument, then `index_substitutions`, then `substitutions` list from the end; guards with `active_const_substitutions.len() >= 64` or already containing the symbol (`:8460-8464`). The 64 limit is a hard cap.
- `index_name` tries `index`, then `integer(program, symbol, spelling)` (decimal parse or canonical atom, `:8764-8777`), then `boolean(...)` (`boolean_literal_spelling` or canonical atom, `:8745-8756`), then conformance record value, then an evaluated array/record/variant const declaration, then `normalize_const_or_nominal_name`. So a `Named` reference with an invalid symbol is classified by its **spelling** ("7", "true", canonical-const atom) (`:8770,8751-8756`).
- `StaticArgument` with a `type_reference` uses `static_operand_from_type_reference(...)` and chooses between `index_reference` and `normalize_type_reference` on the selected symbol's kind (`:8166-8201`).
- `scalar_argument_matches` rejects a declared `TypeParameter` whose kind is not `Const{ type_reference }` naming the expected primitive (`:8505-8511`); for an undeclared operand it calls `prepare_callable_scalar_argument(expected, &operand).is_ok()` (`:8526-8528`).
- `array_length_argument` accepts: type-only operand refused (`:8689`); `StaticArgument` with a `type_reference` and no application, with `const_literal`; everything else becomes `ConstExpression(index_argument ...)` (`:8725-8728`).
- `unbound_array_length` refuses a valid symbol that is not `Const`, `Parameter` or a const/value `TypeParameter` (`:8598-8607`); otherwise a decimal spelling becomes a literal, else a `ConstParameter` atom of the name (`:8609-8616`). `literal_array_length` rejects a value that does not fit `usize` (`:8585-8588`).

**Cases by shape.** Const declaration identity: only `Array`, `Record`, `Variant` decoded values yield `CanonicalConst` with the declared type here (`:8393-8414`); integer and boolean declarations are read through `integer`/`boolean`.

**Fail-open risks.**
- `scalar_argument_matches` returns `true` for any operand that is a declared const-or-value parameter after the checks at `:8505-8511` and no substitution applies (`:8523-8525`): "declared.is_some() => true". Wrong program: a const parameter of the right carrier that no actual binds is treated as matching; this is intended for open generic checks (module comment at `:8478-8480`). Classification: stated.
- `rejected(context)` sets the flags but still returns `UnsupportedConstSubstitution` *text* (`:8732-8743`); a caller that passes no flag cell ignores the failure and receives that text as an identity. Classification: traced (all `applied_*` and exact entry points pass the cells; `type_identity_with_substitution_semantics` at `:10428` and `normalized_const_index_expression_identity` at `:9294` pass only `invalid_index_argument`, the latter returns `None` if set).

### 6.14 Public entry points and `TypedTrees` methods (`:8788-10754`)

**Shape.** Free functions `applied_type_identity` (`:8876`), `applied_type_identity_in_region_scope` (`:8893`), `applied_carrier_identity_in_region_scope` (`:8917`, erases constraints, comment cites domains.md "Carrier-exact membership"), `applied_declared_carrier_identity` (`:8940`), `applied_index_identity` (`:8961`), `portable_applied_index_identity` (`:8977`), `portable_applied_type_identity` (`:9019`), `descriptive_type_identity_in_region_scope` (`:9040`), `applied_lifetime_identity` (`:9100`), `local_trait_application_identity` (`:9150`), `local_trait_contract_identity` (`:9176`), `portable_trait_application_identity` (`:9203`), `normalized_index_identity` (`:9237`), `scoped_*` (`:9255-9308`).
Request structs `TypeIdentityRequest` (`:9311`), `ExactOwnerTypeIdentityRequest` (`:9339`).
`impl TypedTrees` (`:9350-10754`): `closed_data_field_definition` (`:9351`), `closed_data_definition` (`:9381`), `data_constructor_definition` (`:9449`), `data_constructor_case` (`:9502`), `data_constructor_field` (`:9531`), `closed_data_member_field` (`:9594`), `data_field_declaration_symbol` (`:9649`), `file_declaration_path` (`:9750`), `normalized_hermetic_symbol_identity` (`:9767`), `callable_contract_source_path` (`:9844`), `domain_declaration_path` (`:9918`), `closed_requirement_declaration_identity` (`:9944`), `normalized_type_identity` (`:10060`), `domain_application_identity(_in_scope)` (`:10093,10110`), `validate_runtime_membership_application` (`:10171`), `runtime_membership_domain_identity` (`:10226`), `normalized_nominal_type_identity` (`:10249`), `type_identity`, `storage_type_identity`, `exact_owner_type_identity` (`:10451`), `exact_owner_nominal_identity` (`:10492`), `normalized_result_dispatch_set` (`:10527`), `machine_callable_identity_parts` (`:10538`), `trait_requirement_callable_identity_parts` (`:10601`), `operator_callable_identity_parts` (`:10663`), `named_callable_identity_parts` (`:10689`).

**How it decides.**
- Data constructor lookups (`data_constructor_definition`, `_case`, `_field`, `closed_data_member_field`) match a specialized instance's member to its template member by **ordinal position** within `data_members` and then by `name`, `identity` and `relevance` equality (`:9550-9557,9525-9528,9640-9645`). "original.name == selected.name" is a spelling compare among positions (`:9525,9551,9640`).
- `closed_data_definition` for a generic with arguments finds the instance by scanning every data definition and requiring the origin base symbol, no lifetimes at the origin, equal `normalized_type_identity`, and equal lifetime parameter lists; exactly one match required (`:9417-9430`).
- `normalized_hermetic_symbol_identity` (`:9767`) chooses its form by: closed requirement application; derived nominal; Machine kind (needs `machine_declaration_for_lexical_scope` and `machine_callable_identity_text`); trait machine signature; CallableContract (source occurrence path); Domain (subject type path); then package identity; then `SourceOrigin::Toolchain` or `ToolchainContract`; other origins are errors (`:9830-9841`). Keyed on `SymbolKind`, then on source origin.
- `callable_contract_source_path` (`:9844`) uses the *virtual path shape* `<...>` (single component starting `<` and ending `>`) to detect virtual sources (`:9864-9868`), and builds a `source-occurrence-v1:` text with the byte span (`:9905-9908`). A string compare on the path.
- `file_declaration_path` returns the plain path when there is no source span, when the source is toolchain, or when the file lookup fails (`:9751-9761`). Otherwise `"{filename}::{local}"`.
- `exact_owner_type_identity` returns `None` if the missing-owner or invalid flag is set (`:10489`).
- `validate_runtime_membership_application` compares supplied and retained domain arguments by identity (`:10215-10221`).

**Cases by shape.** `domain_application_identity_in_scope` accepts only `Type` and `Const` domain parameters; any other kind sets invalid (`:10147-10157`). `closed_data_field_definition` loops at most `type_reference_count()+1` times (`:9359`).

**Same job elsewhere.** Package-qualified nominal naming appears three times: `qualify_non_binder_name` (`:2532`), `normalized_hermetic_symbol_identity` (`:9767-9841`, a second production that formats `toolchain::{path}` / `toolchain-contract::{path}` / `package_declaration_identity`), and `exact_owner_nominal_identity` (`:10492`, calls `qualify_non_binder_name`). `normalized_hermetic_symbol_identity` does not call `qualify_non_binder_name`, so toolchain declarations are named `toolchain::path` there and by source digest `compound(Nominal,[owner,path])` in the other (read both; suspected inconsistency is a property of the two being separate identity families, not tested here).
The trait-application production (`TraitApplication` compound with Declaration, Arguments, Lifetimes) appears in `local_trait_application_identity` (`:9150`), `portable_trait_application_identity` (`:9203`), `conformances::project` (`:6535-6542`), `portable_record_schema_identity_in_context` (`:6939-6957`).

**Unused / one-caller (grepped).**
- `portable_applied_index_identity`, `descriptive_type_identity_in_region_scope`, `normalized_index_identity`, `portable_applied_lifetime_identity`, `applied_lifetime_identity`, `applied_declared_carrier_identity` together: 40 content matches outside `type_identity.rs` (command: Grep with `glob !**/type_identity.rs`).
  - `portable_applied_index_identity`: 16 call or import sites across 10 files (domain_access_plans, placements, callable_applications x2, private_callback_slots, declaration_reflection x2, declarations x2, product_operand/applications, trait applications/regions x2, declaration_tables_and_index x2).
  - `descriptive_type_identity_in_region_scope`: reflected_fields.rs, declaration_reflection.rs x2, a pipeline 06 test.
  - `portable_applied_lifetime_identity`: `declare_selected.rs:127`, `regions.rs` x2.
  - `normalized_index_identity`: `declarations.rs:1305` only (one call site).
  - `applied_lifetime_identity`: `regions.rs:359` only.
  - `applied_declared_carrier_identity`: no match outside this file (0 call sites in `source-rust/`, grepped). It appears only at its definition (`:8940`).
- `local_trait_contract_identity` (`pub(crate)`, `:9176`): not grepped.
- `requirement_schema_definition`: not grepped.

**Stated limits (quoted <20 words).**
- "Until that complete operation is represented, refuse an exact identity" (`:3370`).
- "Source declarations are owned by a file. The directory namespace is still useful for diagnostics while older import spellings are being migrated" (`:9748-9749`): migration debt about import spellings.
- "A block value is no index term" (`:3412`).
- "A match without a subject is no index term" (`:1446`).
- "An ambiguous declaration cannot supply a guessed empty arity" (`:2633`).

**Fail-open risks (listed; each classified).**
1. `normalize_type_identity` with no `invalid_application` cell in context returns spelled text for unresolved lifetimes, unresolved names, rejected const substitutions (`:2311-2314`, `:8732-8743`). Reached by `TypedTrees::type_identity`, `normalized_type_identity`, `storage_type_identity`, `region_free_scoped_type_identity`, which set no invalid cell (`:10299-10335,10404-10426`). Wrong program: a type with an unresolved lifetime or unsupported const substitution gets an identity (equal to another with the same text) instead of `None`. Classification: traced (not a bug per se; these are the "ordinary" identity for type equality, and callers that need exactness use the other entry points).
2. `closed_data_definition` returns `None` when it cannot find exactly one instance (`:9428-9431`), a refusing direction. `data_constructor_definition` compares template origins using `declaration()` closure; falls through to `Some(symbol)` if the nominal has no `generic_instance` (`:9476-9478`). Classification: traced.
3. `file_declaration_path` returns the bare path on any failure to find the source file (`:9751-9757`); `normalized_hermetic_symbol_identity` then produces an identity without the filename. Wrong program: two same-named declarations in different files whose source file lookup failed would share a path. Classification: open. The later origin check (`:9830-9841`) still requires package or toolchain provenance.
4. `exact_owner_nominal_identity` for a toolchain symbol with no matching `exact_toolchain_sources` entry computes the digest from the declaring file when `missing_exact_nominal_owner` is set (`:2583-2587`); stated by comment "A caller that requires an exact owner but did not pass this source's digest gets the digest of the declaring file itself". Classification: stated.
5. `normalize_index_expression` for `ExpressionNode::Name` uses `substitution::index_name(...)` fallback with `unwrap_or_else` when `substitution::index` returns `None` (`:3356-3359`), so an unbound name is identified by spelling unless the context is structural or has `signature_owners` (checked at `:3351-3355`). Classification: traced.
6. `domain_application_identity_in_scope` returns the declaration identity unchanged if the instance has zero arguments (`:10122-10124`); `domain_arguments_for_instance` returning `None` is an error (`:10119-10121`). Classification: traced.

---

## 7. `TR/type_system.rs` (4,405 lines)

**Purpose.** Defines the type-reference vocabulary and table (`TypeReferenceNode`, `TypeReferenceTable`, `FixedArrayLength`, `TypeConstraintNode`, `DomainConstraint`), numeric constant evaluation for index expressions, and the static type contexts that record proven type equations and case reachability. Header comment (`:1-5`).

**Layout.**

| Lines | Contents |
| --- | --- |
| 7-526 | `pub mod closed_numeric`: exact rational/integer evaluator of anonymous and typed numeric expressions |
| 527 | `pub use crate::source_state::type_identity;` |
| 528-902 | `mod type_predicates`: `StaticTypeContexts`, `TypeWorld`, `ProvenTypeContext`, static type predicates |
| 903-4165 | `pub mod types`: copy and remap, display, substitution, `TypeReferenceTable`, node enums, tests |
| 4167 | re-export `CaseMembershipTypeFacts` |
| 4171-4405 | `pub mod source_projections`: `SourceTypeCursor`, field and element type lookup |

### 7.1 `closed_numeric` (`:7-526`)

**Shape.** `evaluate_anonymous_value<ALLOW_DECIMAL_LITERALS>` (`:47`), `has_anonymous_numeric_results` (`:67`), `TypedTrees::closed_integer_expression_value_in` (`:103`), `closed_integer_value_in` (`:115`), `evaluate_value` (`:136`, an explicit stack machine with `Step::{Enter, Leave, Binary, Cast}`), `apply_numeric` (`:306`), `land_integer` (`:443`), `has_anonymous_operator_meaning` (`:467`), `has_builtin_anonymous_operands` (`:485`).

**How it decides.**
- Anonymous (untyped) literals and operations combine exactly as `BigRational`; `Add/Subtract/Multiply/Divide` only (`:319-325`). With any typed operand the operator must be one of `Add/Subtract/Multiply/Divide/Modulo` (`:328-335`), must have builtin meaning (`has_builtin_spelled_expression_meaning`, `:353-358`), must have equal primitives and equal arithmetic policy (`:359-378`), and is computed in the carrier by policy: `Exact|Trapping` use `exact_*`, `Wrapping` `wrapping_*`, `Saturating` `saturating_*` (`:383-430`).
- `land_integer` accepts a value only if it fits `i64`/`u64` first (`value.to_i64()?`, `to_u64()?`, `:445-447`) and then the carrier's `admits` (`:449`). So carriers wider than 64 bits never land here even though `FixedInteger` allows widths up to 257 (the tests in this file use `u128`, `i257`; read `:3612-3616`).
- A `Match` is evaluated only through `selected_arms`, where the owner must have exactly one distinct selected arm (`:175-186`).
- Name lookup goes to `const_declaration_by_symbol(path.symbol)` and evaluates `materialized_initializer` then casts to `declared_type` (`:242-248`); only with `typed_owner`.
- Builtin operator meaning for anonymous operands: every authored selection occurrence on the expression must be a `Call` to a Machine/State/Operator, a `DomainMembership` to a Domain, an `Intrinsic::BuiltinOperator|CoreIntegerOperator`, a late-bound `CheckedOperator`, or a resolved `Const` (`:497-523`).

**Cases by shape.** "Only `Add, Subtract, Multiply, Divide`" in `has_anonymous_operator_meaning` (`:475-481`); `Modulo` is excluded there. `literal_carrier` returns `None` for `LandedIntegerType::Addr` (`:456-466`), so an address-landed literal makes the evaluation fail.

**Same job elsewhere.** Constant folding of integer/arithmetic expressions also exists in `crate::source_state::const_integer::closed_literal` (called at `TR/type_identity.rs:8630`) and in `constant_recipes` (`closed_constant_recipe_value`, called at `:109`); three evaluators for closed integers inside the typed-tree crate (read the calls, not the other two bodies; suspected).

**Unused / one-caller (grepped, over `source-rust/`).** `evaluate_anonymous_value|has_anonymous_numeric_results|closed_integer_expression_value_in`: 16 matches in 8 files (including 4 in this file). `evaluate_anonymous_value` is called from `has_anonymous_numeric_results` here; the other names have external callers in pipeline 03 (`operator_calls.rs` x3, `domain_cast_accounting.rs`, `typing_queries.rs`), pipeline 05 (`read/contracts.rs` x2, `read/types.rs` x2), `operator_declarations.rs` x2, `normal_memberships.rs`.

**Fail-open risks.** `apply_numeric` with `typed_owner = Some(machine)` and an invalid `machine` handle runs the "any specialization selects a trait operator meaning" check and returns `None` (refuse) if any is found (`:337-352`); without such a specialization it proceeds. Classification: traced (the comment is absent; code returns `None` on any selected trait meaning in any specialization, which refuses).

### 7.2 `type_predicates` (`:528-902`)

**Shape.** `StaticTypeContexts` (`:618`, vectors keyed by expression handle, sorted by `(arena_index, generation)` for binary search): `boolean_equalities`, `completed_case_values`, `admitted_match_cases`, `case_memberships`, `rows: Vec<(ExpressionHandle, Vec<TypeWorld>)>`, `statement_rows`, `transition_rows`. `TypeWorld { bindings, exclusions }` (`:635`). `ProvenTypeContext` (`:786`). `TypedTrees::retain_proven_type_contexts` (`:795`), `admitted_match_cases` (`:884`), `statically_refined_type_reference` (`:776`), `applied_static_type_predicate_value` (`:566`).

**How it decides.**
- `refined_in_worlds` returns the original reference unless **all** worlds resolve it to the same identity (`:656-675`). If there are no worlds (unreachable) it returns the reference (`unwrap_or(reference)`, `:674`).
- `TypeWorld::constrain` (`:727`): equal identities satisfy only an `equal` constraint; a binder (a `TypeParameter` symbol) on either side gets bound or excluded; two closed non-equal types satisfy only a `not-equal`; two open non-binder types are kept as is (`return Some(self)`, `:757`). An equality that contradicts an exclusion returns `None` (world dies, `:760-766`).
- `retain_proven_type_contexts` joins contexts for the same expression: if both reachable, `admitted_cases` union (or `None` if either is `None`), `case_value` kept only if equal, `equations` intersected (`:808-825`); an unreachable previous takes the new context's data (`:826-830`); an unreachable new context does nothing (`:808`) - the joined row stays as it was.
- `applied_static_type_predicate_value` evaluates `==`/`!=` of two `TypeExpression`s by comparing their identity under the given substitutions, `&&`, `||`, `!`, Boolean; any other shape returns `None` (`:571-606`). Requires both operands closed under the substitutions (`:572-575`).

**Cases by shape.** `static_type_predicate_operands` requires a `Binary` with `Equal|NotEqual` and both sides `TypeExpression` (`:543-562`).

**Fail-open risks.**
- `TypeWorld::constrain` line `:752-757`: two unequal open, non-binder types return `Some(self)` (keep the world with no new constraint). Wrong program: an equation between two non-binder open types contributes nothing and does not make a world unreachable. Classification: open (no comment; plausibly "unknown stays satisfiable", the refusing/non-refining direction for types since refinement only narrows).
- `refined_in_worlds` on an empty world list (`:674`) returns the unrefined reference, not "unreachable"; `is_unreachable` is separate (`:677`).

### 7.3 `types` (`:903-4165`)

**Shape.**
- `copies` (`:913-1272`): `TypeCopy::ty` (`:926`) copies a type graph into another table, memoizing in `self.types` (a `Vec` searched linearly, `:930-934`); `TypeRemap::ty` (`:1142`) rewrites symbols through `remapped`.
- `display` (`:1400-1540`): `display_name`, `display_name_with_constraints`.
- `substitution` (`:1669-2077`): `substitute_node`, `substitute_from`, `substitute_applied_from`, `substitute_occurrence`, `renamed_row`, `substitute_shared_children`, `instantiate`, `instantiate_arguments`.
- `TypeReferenceTable` (`:2113`): 15 parallel arenas keyed by the same handle (`type_references`, `type_reference_handles`, `constraints`, `constant_origin_spans`, `constant_origins`, `array_extent_arguments`, `operand_lifetime_owners`, `operand_lifetime_spans`, `operand_lifetimes`, `operand_origins`, `type_alias_applications`) plus four derived indexes `NamedRowIndex`, `ArithmeticRowIndex`, `SliceViewIndex`, `PolicyShellIndex` (`:2125-2129`).
- Enums: `TypeReferenceNode` (9 variants, `:3157`), `FixedArrayLength` (4, `:3209`), `TypeConstraintNode` (3, `:3314`), `DomainConstraintSubject` (3, `:3342`), `ValueDomain` (`Finite` only, `:3322`), `OmegaLayoutGrammar` (`Derived` only, `:3352`), `DomainConstraint` (`:3365`), `TypeOperandOrigin` (`:2097`), `ProductTypeInput` (`:2090`).

**How it decides.**
- Type rows have no structural interning: `insert` always appends (`:2241-2266`); identity is not row identity. Dedup exists only for `retain_type_alias_application` (linear `any(row == &application)`, `:2294-2305`).
- Derived indexes compare `PartialEq` as always-equal (`impl PartialEq ... fn eq -> true`, `:2156-2218`), so table equality ignores them. They write nothing in records (`:3424-3478`).
- `find_named_type_reference` picks the lowest arena index per symbol (`:2906-2923`, `:2485-2495`); `find_arithmetic_result_type_reference` indexes constrained rows with exactly one `ArithmeticDomain` constraint (`:2958-2962`); `find_policy_qualified_type_reference` is the same keyed by target handle (`:2979-3050`, guarded by append epochs `:2992-3014`). These return the *first* matching row.
- `substitute_occurrence` refuses if constant origins of either side are stale (`:1744-1745`), keeps the original `operand_lifetime_owner`... it overwrites with the replacement's (`:1752-1756`).
- `instantiate` rebuilds a node only if a child changed (`(instantiated != x).then_some(...)`, e.g. `:1939,1973,2011,2020`); `Callable` is refused: "a machine contract requires TypedTrees::instantiate_type_reference" (`:2037-2040`); a `FixedArray::ConstParameter` substituted by a `Named` with an invalid symbol is parsed through `anonymous_numeric_literal_value` -> `usize` or refuses "an applied array length is not a closed nonnegative integer" (`:1999-2003`).
- `retain_operand_lifetimes` accepts an empty roster or one whose length equals the node's written lifetimes (`:2436-2438`).
- `primitive_type`: a `Named` symbol maps through `symbols.primitive_type(*symbol)` (by builtin atom, not by spelling; the test `scalar_identity_survives_names_qualifications_and_atomic_wrappers` asserts that a `Data` symbol named "u32" is not primitive, `:3629-3728`).
- `arithmetic_domain`: first `ArithmeticDomain` constraint on a `Constrained`, else the base's; `Reference` delegates to the referee; everything else `Exact` (`:3267-3286`).

**Cases by shape.**
- `retain_collection_result_types` (`:2852`) handles only `Reference`/`Constrained` wrappers around a `Slice` or `FixedArray`; any other shape returns without adding views (`:2858-2868`). It then adds `Shared` and `Mutable` slice-view reference rows (never `WriteOnly`, `:2819,2888-2891`) for the element if absent.
- `element_inherits_collection_arithmetic_policy` returns true only for integer/address/F32/F64 builtins, or a nested array or slice (`:3052-3080`).
- `substitute_shared_children` rewrites only `Reference`, `Constrained`, `FixedArray` element, `Slice` element, and `Generic` argument slots that are `Named` of the given parameter symbol (`:1828-1898`); other shapes are untouched. Rewrites mutate shared rows in place from `type_start` forward.

**Same job elsewhere.** `TypeReferenceTable::instantiate` (`:1918`) and `TypedTrees::instantiate_type_reference` (outside the unit, named in the error at `:2039`) both substitute; the first refuses callables. `TR/data_applications.rs` uses the first (`:414,422,431`). `TypeReferenceTable::substitute_occurrence` and `TypeCopy::ty` both copy the metadata columns (origins, lifetimes, alias applications) one column at a time; there are about 8 columns copied at `:1032-1083` and again at `:1749-1792` and `:2307-2341` (`fresh_occurrence`) (read).

**Unused / one-caller.** Not individually grepped. Named for the next reader: `has_resolved_operand_lifetimes` (`:2400`) is called at `TR/type_identity.rs:2194`.

**Stated limits.** "Type occurrences and their aligned metadata retain one absolute slot numbering across continuations. Appending a new body must not copy the types already available to it" (`:2110-2112`). `debug_assert_eq!` on aligned arena indices at `:2252,2256,2259-2261`.

**Fail-open risks.**
- `TypeCopy::ty` uses `get_mut(...)` assuming the copied row exists; the aligned-arena assertions are `debug_assert!` only (`:2252-2261`), compiled out in release. Classification: open.
- `constant_origins` returns `Err("constant type origin has a stale receiving type")` for stale handles; `attach_constant_origins` propagates (`:2445-2483`). Refusing.
- `substitute_node` does not check the new node's children exist. Classification: open (internal API, all callers in typing/preparation).
- `anonymous_numeric_literal_value` accepts any `Named` with an invalid symbol whose spelling parses as `usize` (`:2588-2599`): a spelling decides. Classification: traced (generated names only; `Identifier::generated` in `data_applications.rs:383-389` and `:107`).

### 7.4 `source_projections` (`:4171-4405`)

**Shape.** `SourceTypeCursor` (`:4176`) with `new`, `current`, `field`, `element`; `field_type_reference` (`:4224`), `indexed_element_type_reference` (`:4304`), `substituted_type_reference` (`:4347`), `data_field_type_reference` (`:4381`).

**How it decides.**
- `field_type_reference` peels `Reference`/`Constrained`, then for a `Generic` takes `data_application_definition(type_reference)` when the substitutions list is empty *or* the type's identity equals its identity under the substitutions (`:4245-4254`), else `closed_data_definition`, else binds only `TypeParameterKind::Type` parameters to their (substituted) arguments (`:4263-4281`). For a `Named` it looks for a machine with attached data (`program.machine(*base_symbol)`), else a unique data definition (`:4288-4294`). `Callable`, `ConstExpression`, `FixedArray`, `Slice`, `Unit` return `None` (`:4296-4300`).
- `data_field_type_reference` matches a field by `field.symbol == field_symbol` in members and variant payloads (`:4393-4403`), after requiring `field_symbol.is_valid()` (`:4386`).
- `substituted_type_reference` follows the binder chain a bounded number of times (`substitutions.len()+1`, `:4355`) and stops if a replacement equals the current reference (`:4373`).

**Cases by shape.** `field_type_reference` projects through `Generic`, `Named` data, and `Named` machine with attached data application (`attached_data_field_type_reference`, `:4213`). Only `Type`-kind parameters are bound when stepping into an unpreferred generic (`:4272`); const and value parameters of that generic are **not** bound (the filter drops them), so a field whose type mentions a const parameter keeps its unsubstituted reference. Classification of that as a limit: read, not tested here.

**Fail-open risks.** none seen; every failed lookup returns `None`.

---

## 8. `typed_trees.rs` (221 lines; crate root of `typed-trees`)

**Purpose.** Declares `TypedTrees`, the one forest for checked declarations and bodies, and its smaller records. Module comment `:1-3`.

**Shape.** `TypedTrees` (`:20`) has 49 fields: identity, `roots`, `tables`, `symbols: SymbolTable`, service reaches and rows, semantic domains, applied domain applications, opaque representation selections (`Vec<OpaqueRepresentationSelection>` "ordered by the opaque declaration's symbol", `:36-38`), external bindings, plan-laid layouts, access view plans, wire placements and plans, machine specializations, captured machine applications, trait/data implementation applications, `deferred_refusals` ("Requests no pass served, on the declarations that hold them", `:57-59`), declaration indexes, `pending_const_range_endpoints: std::collections::HashSet<ExpressionHandle>` (`:65-66`), constant recipes and closure, `callable_identities: OwnedInterner<CallableIdentityTable>` and `type_identities: TypeIdentityTable` (`:71-73`). Derives `Debug, Default, PartialEq, Eq, record_storage::Record` with `write_scope = type_identity::write_under_own_numbering` (`:18-19`).
`ParkedBuildLocals` (`:77`), `ProgramIdentity(u64)` (`:118`: `Default` takes a process-unique counter, `PartialEq` always true, `Record` writes nothing, `:126-151`), `EvidenceForwarding` (`:154`), `ProofOutputCall` (`:165`), `DataApplicationView` (`:182`), `DesignatedStaticTypeArgument` (`:188`), `remapped` (`:201`), `escape` (`:208`).
Modules attached at `:105-115`: `callables`, `definitions`, `exports`, `expressions`, `proofs`, `storage`, `type_records`, `table_macros`. `pub use crate as source_state` (`:10`), `pub mod typed_trees { pub use crate::source_state::*; }` (`:219-221`).

**How it decides.** `deferred_build_subject` (`:89`) compares a symbol's source span `(source_id, start, end)` to sorted spans in `parked_build_locals[*].deferred_subjects` by binary search (`:93-100`): keyed on source span, not symbol identity. It assumes each `deferred_subjects` vector is sorted by that key; nothing in this file enforces that.
`escape` (`:208`): backslash-escapes `\ ( ) ,` in a string; used for identity-like text (callers not grepped).

**Unused / one-caller.** `deferred_build_subject`, `escape`, `proof_fact_source_span_index` (`:194`), `DesignatedStaticTypeArgument`, `DataApplicationView`: not grepped.

**Stated limits.** `#![allow(clippy::obfuscated_if_else, reason = "valid-handle copy expressions use one uniform then/fallback idiom")]` (`:5-8`).

**Fail-open risks.**
- `deferred_build_subject` returns `false` when a symbol has no source span (`:90-92`) and relies on sorted order; an unsorted vector gives false negatives. Wrong program: a Build-subject machine that is not recognized as deferred. Classification: open.
- `ProgramIdentity::eq` is always `true` (`:133-137`), so two forests that differ only by identity compare equal; documented in module-level orientation only as "ignored by equality" in the crate map; no comment here. Classification: stated by the crate map, not at the site.
- `HashSet` field (`:65-66`): the AGENTS.md ratchet names dense-id maps; I did not check `tools/ratchets/dense_id_maps.txt`.

---

## 9. `source_to_psi.rs` (211 lines; driver crate root)

**Purpose.** The public entry of the source half of the compiler: check, compile, compile for a target, review a package, inspect one machine. Header: "Check or publish Omega source as Psi. Each request below names its stopping point or entry" (`:1-2`).

**Shape.**
- `StageSource<'a> { parsed: Box<ParsedStageSources>, product: ProductSourceInputs<'a> }` (`:44`).
- `check(source, timings) -> Result<Vec<Diagnostic>, Vec<Diagnostic>>` (`:51`): `source.resolve`, `product.judge(..., ProductEntry::BuildBindings, ...)`, then `checked.terminal_publication.check(...)`.
- `compile(source, advisories)` (`:68`) -> `driver::publish_stage(source, ProductEntry::BuildBindings, ...)`.
- `compile_for_target(source, target, advisories)` (`:81`) -> `publish_deployment` then `driver::published_target`.
- `review_package(source, target, accounting)` (`:96`) with `ProductEntry::PackageReview`; builds `CheckedPackageDeclarations` (`:151`).
- `inspect(source, machine)` (`:203`) with `ProductEntry::Inspection(machine)`.
- `publish_deployment` (`:126`).
- Re-exports at `:8-31`.

**Inputs / outputs.** Consumes `StageSource`; produces `PublishedPsi = Arc<CanonicalTerminalArtifact>` (`:40`), `PublishedTargetProgram`, or diagnostics. `CheckedPackageDeclarations` takes `std::mem::take(&mut checked.declarations.service_audits)` (`:118`) and clones `explicit_providers`, `native_supplies`, `grants` (`:116,117,119`).

**How it decides.** `review_package` refuses with "package review requires a named source subject" when `source.product.subject.is_none()` (`:101-105`). `publish_deployment` refuses with "source program did not select deployment target `{target}`" when `!source.product.calling_targets.contains(&target)` (`:140-144`): keyed on a target **string** compared against the list. `has_assumed_bindings` is `!checked.declarations.assumed_bindings.is_empty()` (`:120`).

**Same job elsewhere.** `check`, `compile`, `inspect` are three thin wrappers over `ProductEntry` variants (read). `source.product.calling_targets.contains(&target)` is a string match; the target-name list is also matched elsewhere in the build crate (suspected).

**Unused / one-caller (grepped).** `compile_for_target`: callers `build/planning/product.rs:267` and tests (`default_build.rs`, `provider_publication.rs`, `fixed_image.rs` x4+). `review_package`: `build/planning/package_review/mod.rs:68`, `provider_publication.rs:1082,1379`. `inspect`: `application/src/inspection/mod.rs:56` (via `compiler::inspect`). `check`: the grep for `source_compiler::check` found no occurrence (0); callers via other import spellings not grepped, so unknown. Commands: Grep for `\binspect\(|compile_for_target\(|source_compiler::check|review_package\(` over `source-rust/`, head-limited to 20 lines (the listing was truncated; counts above are from visible lines only).

**Stated limits.** "Source requests own the parser tables. Boxing them keeps saved Psi requests small" (`:45`).

**Fail-open risks.** none seen. `check` returns advisories on success. `CheckedPackageDeclarations::accepted()` and `supplied_declarations()` propagate `Result` (`:184-200`).

---

## 10. `test_support.rs` (45 lines; driver crate)

**Purpose.** Re-exports raw stage operations under `source_compiler::test_support` for fixtures: `lexing`, `parsing`, `resolution`, `typing`, `preparation`, `lowering`, `checking` and `TerminalSourceCompilation` (`:2-9`); `layout_target_scopes`, `FrozenSource`, `judge_terminal_source`, `SourcePreparationRequest`, `ResolveAndTypeRequest` (`:11-14`); and two functions, `type_authored_source_for_test` (`:17`) and `prepare_source_for_test` (`:29`). Under `#[cfg(test)]` it mounts three files by `#[path]`: `tests/support/fixture_repository.rs`, `tests/support/frontend_operations.rs` (as `frontend_fixture`) and the local `tests/support/schema_customer_source.rs` (`:35-45`). Compiled with `#[cfg(any(test, feature = "test-support"))]` (`source_to_psi.rs:24`).
Fail-open: none seen. Unused: the module is a re-export list; not grepped.

---

## 11. `tests/stage_source.rs` (918 lines; driver integration tests)

**Purpose.** Tests the public source boundary (`compile`, `StageSource`, `BuildStageSource`, `CheckedBuildInvocation`) with hand-captured sources.

**Shape.** Helpers `capture_file` (`:17`), `captured` (`:37`), `product` (`:43`: a `StageSource` with `build_host: ProductTarget::LinuxX86_64`, `targets: &[]`, `calling_targets: &[]`, defaults elsewhere). `publish_build` (`:448`).
Tests (name, positive/negative):
- `an_entryless_checked_product_publishes_reopenable_psi` (`:73`): positive; reopen bytes equal.
- `inherited_provider_methods_keep_the_declaring_application_and_result` (`:88`): positive then negative twin by editing `u32` to `u64` in the body (`:130-135`).
- `selected_requirement_reach_keeps_its_receiver_arguments` (`:139`), `const_service_terms_are_values_in_the_callers_scope` (`:169`): positive only.
- `a_generic_call_forwards_the_callers_const_service_argument` (`:201`): positive, negative by changing `Sensor<29u64>` to `Sensor<17u64>` (`:212-217`).
- `invocation_ceiling_follows_callable_parameter_aliases` (`:221`): positive bodies and negatives checking message contains `invokes` and `parameter 2` (`:262-269`).
- `selected_members_keep_the_callable_type_they_share` (`:274`): compiles `samples/cli/facts/dynamic_trait_dispatch/local/main.omg` via `include_str!`.
- `omega_compiler_storage_and_outcomes_publish_psi` (`:289`): compiles `source/subsystems/compiler/request/request.omg`, `outcome.omg` and the storage half of `source_files/source.omg` (split at `"public machine source_set_add("`, `:295-297`).
- `complete_omega_request_protocol_publishes_psi` (`:313`): `#[ignore = "checks the complete request decoder and encoder together, which takes several minutes"]`.
- `omega_frontend_imports_a_member_of_the_lowered_program_representation` (`:366`): positive and negative twin (`ScalarLayout` vs `ScalarParameter`); the negative asserts a message "type without a symbol while reading the signature of `handle`" (`:409-412`).
- `a_description_for_the_same_name_and_span_in_another_package_is_refused` (`:417`): negative; message contains "retained declaration is absent or changed".
- `build_and_product_check_the_return_value_before_publication` (`:482`): `ensures result == 7u32`; 7 passes, 8 fails with an `ensures` diagnostic at stage `Checking` and span of the returned literal, for both Product and Build.
- `a_build_activation_rejoins_its_authored_machine_in_the_selected_package` (`:525`): positive and negative (foreign package digest).
- `publication_checks_the_same_grammar_in_root_and_imported_files` (`:566`): four negatives with expected message substrings "`module` headers", "comparison", "between arms" x2.
- `a_computed_array_extent_is_refused_where_another_length_is_required` (`:636`): exact refusal text and span (`:681-701`); doc comment cites `wiki/spec/language/constants.md`, "Evaluation".
- `parser_output_must_cover_the_exact_captured_files` (`:706`): negatives "does not match its selected source occurrence" and "missing from the parsed stage".
- `a_file_qualifier_selects_its_own_sources_declarations` (`:742`), `file_imports_neither_bind_bare_names_nor_reexport_imports` (`:781`), `file_qualifiers_respect_foreign_declaration_visibility` (`:833`; three cases: public+direct ok, not direct "direct", private "private"), `a_static_case_in_a_file_uses_the_type_even_when_a_parameter_has_its_name` (`:870`), `a_file_qualified_domain_still_requires_its_declared_carrier` (`:894`).

**How tests decide.** Negative tests assert on **message substrings** (`contains("bool")`, `contains("read")`, `contains("Tag")`, etc.); only the array-extent test pins the full text.

**Unused / stated limits.** The ignored test's attribute text is the limit. `include_str!` paths reach into `samples/cli/facts/` and `source/`.

---

## 12. `tests/final_stage_validators.rs` (122 lines; `compiler` crate)

**Purpose.** Two tests: `saved_psi_requests_do_not_store_the_parser_tables_inline` (`:16`): `size_of::<Program>() < size_of::<ParsedStageSources>()` and same for `CompileRequest`. `final_native_image_passes_checks_and_runtime_admission_obeys_applicability` (`:23`): loads `tests/fixtures/calls/run/scalar_contract_route_exit/linux_x86_64.psi` (`include_bytes!`, `:24-26`), authorizes it with `HostProducerTrust::accept("published scalar contract route fixture")` (`:28`), compiles to native for `TargetProfile::LinuxX64` with the named validator `FixedFrameCorrespondence`, expects 1 row `Passed` with target name; then a `CompileRequest::Runtime` with all validators expects 3 rows each `Inapplicable("runtime admission stops before target lowering")`, and a runtime request naming `FixedFrameCorrespondence` returns `NativeValidationError::Unavailable` with `halt_value() == 1` (`:108-121`).
Only LinuxX64 is exercised (`:29`).

---

## 13. `tests/support/schema_customer_source.rs` (152 lines)

**Purpose.** Test helper that loads the real `source/library/core` units plus selected `alloc` units (`main.omg`, `bump/bump.omg`, `vec/vec.omg`, `:25`) into a `SourceMap`, parses them with the fixture front end, builds `SourceScopedTopLevelBinding::logical_module_import` bindings for every `use omega::language::core...` found (`:107-129`), resolves and types them (`:132-138`, `source_with_core` `:31-40`). Header: "This supplies no package-lock admission or private policy-selection authority, and inserts no closed application, checked tree or catalogue row" (`:1-4`).

**Shape.** `actual_library_source` (`:16`), `source_with_core` (`:31`), `resolved_library_source` (`:42`), `retained_units` (`:140`).

**How it decides.** `retained_units` walks `core` recursively and keeps every `.omg` file whose name is not `build.omg` (`:145-148`): a file-name compare. Import detection compares the first three path members to `"omega"`, `"language"`, `"core"` (`:111-115`): a spelling compare. Core is `SourceOrigin::Toolchain` with `None` package; alloc units and the query use `SourceOrigin::User` with the supplied package (`:63-70,79-85,88-94`).

**Fail-open risks.** `core_anchor` is the first sorted core path (`:72-75`); import bindings of the core path use `core_anchor` for every core import (`:123-128`), so any `omega::language::core::<x>` path binds to that single anchor (stated at `:121-122`, "its anchor creates no module").

**Stated limits.** "The real core stays a distinct toolchain dependency in both forms" (`:30`).

---

## Coverage

All thirteen ranges were read in full, in order:

| File | Lines read in full | Skimmed | Not read |
| --- | --- | --- | --- |
| `TR/data_applications.rs` | 1-470 (470) | 0 | none |
| `TR/data_applications/tests.rs` | 1-134 (134) | 0 | none |
| `TR/mod.rs` | 1-8 (8) | 0 | none |
| `TR/opaque_representations.rs` | 1-224 (224) | 0 | none |
| `TR/source_type_applications.rs` | 1-528 (528) | 0 | none |
| `TR/type_identity.rs` | 1-11561 (11,561) | 0 | none |
| `TR/type_system.rs` | 1-4405 (4,405) | 0 | none |
| `typed_trees.rs` | 1-221 (221) | 0 | none |
| `source_to_psi.rs` | 1-211 (211) | 0 | none |
| `test_support.rs` | 1-45 (45) | 0 | none |
| `tests/stage_source.rs` | 1-918 (918) | 0 | none |
| `compiler/tests/final_stage_validators.rs` | 1-122 (122) | 0 | none |
| `compiler/tests/support/schema_customer_source.rs` | 1-152 (152) | 0 | none |

Total read in full: 18,999 lines. Skimmed: 0. Not read: none.
Detail level varies: the large test-only blocks of `type_identity.rs` (`:4481-6260`, `:10790-11561`, and the in-module tests) and the test blocks of `type_system.rs` (`:3480-4164`) are summarized by what they assert, not line by line, in this record; they were all read.
Greps run (all over `C:/Development/Omega/.agents/worktree/worker-2/source-rust`, Grep tool): `settle_const_argument` (count), `data_applications::prepare_from|prepare` (content), `rederived_selected_application_commitment` (count), the `portable_applied_*` family (count, then content excluding `type_identity.rs`), `evaluate_anonymous_value|has_anonymous_numeric_results|closed_integer_expression_value_in` (count), `\binspect\(|compile_for_target\(|source_compiler::check|review_package\(` (content, `head_limit` 20, truncated).

## Five things a maintainer most needs to know about this unit

1. **`type_identity.rs` is one flat 11.5k-line file holding about 15 nested modules; its identities are strings.** Equality of types, callables, regions and const arguments is equality of interned text (`TypeIdentityTable`, `:3965`; `PartialEq` compares only `texts`, `:4010`). The text is built by one recursive normalizer driven by a 30-field `TypeIdentityContext` (`:2014-2067`). Whether an unresolved name or lifetime is refused or spelled out depends on which `Cell<bool>` fields the *caller* put in the context; the plain `type_identity`/`normalized_type_identity` entry points set none (`:10299,10428-10449`), so they never refuse.
2. **A name with an invalid symbol is classified by its spelling.** `normalize_const_or_nominal_name` (`:3657-3673`), `substitution::index_name` (`:8364-8415`), `integer`/`boolean` (`:8745-8785`), `TypeReferenceTable::anonymous_numeric_literal_value` (`type_system.rs:2588`) and `DomainConstraint` alias fallback (`:1104`) all read decimal text, `true`/`false`, or canonical const atoms from a `Named` node's string when the symbol handle is invalid. `data_applications.rs` writes such nodes itself (`:107,383-389`). Binder lookup by spelling also exists in `context.name` (`:2512-2521`) and lifetime lookup by name string (`:3841-3845`).
3. **Several things are done twice or three times inside this unit.** Package-qualified nominal naming (`qualify_non_binder_name` vs `normalized_hermetic_symbol_identity`, which formats `toolchain::path` itself, `:9830-9834`); the `TraitApplication` text production (4 places, `:9150,9203,6535,6939`); "find the unique binding for a parameter, then compare identity text" (3 places, `:6321,7174,7457`); copying the aligned metadata columns of a type row (`type_system.rs:1032-1083,1749-1792,2307-2341`); integer constant folding (`closed_numeric`, `const_integer::closed_literal` at `type_identity.rs:8630`, and `constant_recipe` closure at `type_system.rs:109`). The file's own comment says its elision rule duplicates stage 05's (`:2069-2074`).
4. **Several functions return an "empty/invalid" value instead of failing.** `machine_callable_identity` returns an invalid id for a machine with no states (`:10542`, `:738`); `callable_identity_text` and siblings return `""` for an unknown id (`:765-809`); `TypeIdentityTable::by_text` returns the invalid id (`:4171`); `boundary_operator_requirement_callable_identity` returns the default for non-boundary operators (`:687`). Which callers check `is_valid()` was not traced. In the same family, `collect_declared_result_dispatch_terms` drops predicate-only domains and returns silently on an alias cycle (`:7649,7682-7688`), which changes overload identity.
5. **Tests in this unit pin behavior by message substring and by fixtures from other directories.** `stage_source.rs` mostly asserts `message.contains(...)` (`:262-269,409-412,441-444,617-621,768-776`); it `include_str!`s `samples/cli/facts/dynamic_trait_dispatch/local/main.omg` (`:278`) and `source/` Omega files (`:284-286,292-294,336,343,377,386`), so edits to those change these tests. `final_stage_validators.rs` covers LinuxX64 only (`:29`). One heavy test is `#[ignore]` (`stage_source.rs:312`). `rederived_selected_application_commitment` has no non-test caller (grepped, 3 occurrences all in `opaque_representations.rs`), and `applied_declared_carrier_identity` has no caller outside its own file (grepped).

## What you are unsure about

- Whether callers check the validity of the "invalid id" returns in point 4; I did not trace them.
- When `machine_instantiation(...)` returns `Ok(None)` for a machine that was in fact specialized (which would make `PortableMachineApplications::identity` return `Ok(None)` at `:7293-7298` and so give an unspecialized-looking name). I did not read `machine_instantiation`.
- Whether `normalized_hermetic_symbol_identity`'s `toolchain::{path}` form and `qualify_non_binder_name`'s `ToolchainSource(digest)` form are meant to be distinct identity families or are an inconsistency; both are read, neither is tested against the other here.
- Whether the `HashSet` field (`typed_trees.rs:65`) and the `std::collections::HashMap` fields (`type_identity.rs:165-169`, `4281-4285`, `type_system.rs:2185`) are listed in `tools/ratchets/dense_id_maps.txt`; not checked. (`CallableIdentityTable` aliases `arena::HandleKeyMap` as `HashMap` at `:27`, so most of its maps are not `std` maps; only `result_dispatch_slots` is `std::collections::HashMap`, `:166`.)
- The `inspect`/`check` caller counts in section 9 come from a head-limited (20-line) grep, so the lists are partial; `check` had no `source_compiler::check` hit, but other spellings were not searched.
- The unit label includes areas 06 and 15 but contains no package-manager files; I treated that as an artifact of the unit assignment, not as missing files (the TSV has 13 rows and I read all 13).
- The brief asks me to read the crate map for my crates first; I read `05-source-representations.md` only. I did not read `06-source-driver-build.md` or `15-package-manager.md` because no row in this unit lies in those crates (the `source_to_psi.rs` and `test_support.rs` rows are the driver crate, which section 4 of the area-05 map describes as belonging to another area).
