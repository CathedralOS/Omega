# slice_0138

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A1061 | omega-rust/psi/compilation/representations/typed-trees/src/product_and_trait_declarations.rs:419-421 | REACHABLE | Missing specialization and skipped operands yield no static environment; trait walks and refinements omit parents and inherited reach without failing. |
| A1062 | omega-rust/psi/compilation/representations/typed-trees/src/evidence_and_inspection.rs:1391-1404 | REACHABLE | Invalid constructors with the same type name share one fallback label; proof-only classification keys arena index only. |
| A1063 | omega-rust/psi/compilation/representations/typed-trees/src/service_declarations.rs:164 | REACHABLE | Ambiguity, missing identities, and duplicate origins become absent catalog rows with no diagnostic. |
| A1064 | omega-rust/psi/compilation/representations/typed-trees/src/lib.rs:1572 | REACHABLE | Authored selections with no source file still pass `checks_authored_selection` and enter checking and reference closure. |

## A1061

- Read `capture_static_environment` (`product_and_trait_declarations.rs:411-577`, read): at `:419-420` (read) no `machine_specializations` match for the instance returns `Ok(None)`, not `Err`.
- At `:465-472` (read) `StaticMachineBindingValue::RuntimeValue { .. }` uses `continue`, so those binders never become captured operands; remaining bindings can still reach `Ok(Some(Arc::new(...)))` at `:576` (read).
- Caller search `capture_static_environment` in `omega-rust`: 6 matches — 2 in `product_and_trait_declarations.rs` (definition + re-export at `:411`, `:1219`), 2 in `build_bindings.rs` (`:300` use, `:469` call), 2 in `build_machine.rs` (`:1662` use, `:1902` call) (read: grep count mode); **2 production call sites** (`build_bindings.rs:469`, `build_machine.rs:1902`).
- Read `build_bindings.rs:462-474` (read): when designated operands include an application, `transpose()?` then `flatten()` turns `Ok(None)` into `static_environment: None` on the terminal binding row without error.
- Read `build_machine.rs:1897-1906` (read): same operand flag; `map_err(Halt::Trap)?` only on `Err`, not on `Ok(None)`.
- Read `AppliedTraitScope::visit_inherited` (`product_and_trait_declarations.rs:3986-4024`, full function, read): at `:4003-4005` and `:4016-4017` (read) missing parent trait definition or failed `Self::new` uses `continue`, so parent traits are dropped from inherited walks; `visit` at `:3974-3976` (read) routes all trait tree walks through this helper.
- Caller search `\.visit\(program` on `AppliedTraitScope` (read): production uses include `service_declarations.rs:154`, `:199`, `:1283`, `validation/src/traits.rs:6279`, `validation/src/contract_entailment.rs:18585`, `build-time-evaluation/src/product.rs:1570`, and `pipeline/04_...` passes (read: grep, not every body opened).
- Read `apply_callable_refinements` (`product_and_trait_declarations.rs:2700-2775`, read): at `:2737-2738` (read) `TraitRefinementReach::Inherited` uses `continue`, skipping service-reach merging for that refinement clause.
- Caller search `apply_callable_refinements` in `omega-rust`: 4 matches — 1 definition, 3 in `callable_applications.rs` (`:4379`, `:6784`, `:7467`) (read).
- Read `callable_applications.rs:7464-7468` (read): refinement errors propagate as `Result` from contract materialization; inherited-reach skips do not error.
- Wrong program (plain words): a Build helper machine is selected with designated static machine operands, but it has no specialization receipt (or only `RuntimeValue` bindings); compilation and terminal production still emit the binding with no static environment, and trait or refinement walks never see unresolved parent traits or inherited reach rows that should have been enforced.

## A1062

- Read `render_proof_constructor_value` (`evidence_and_inspection.rs:1391-1404`, read): calls `constructor_values::render` and on `None` uses `format!("invalid-constructor:{}", literal.type_name)` at `:1403` (read), so the fallback label depends only on `literal.type_name` text, not on case symbol or field values.
- Read `constructor_values::render` (`evidence_and_inspection.rs:663-735`, read): returns `None` when multiple data definitions share `owner` (`:675-676`), case is ambiguous (`:685-686`), field binding is ambiguous (`:714-715`), or duplicate field identity keys (`:724-725`) (read).
- Caller search `render_proof_constructor_value` in `omega-rust`: 4 matches — definition plus call at `evidence_and_inspection.rs:1831`, `pipeline/05_typed-trees-to-checked-trees/src/published.rs:425`, `checks_contracts.rs:10877` (read); the checks_contracts site feeds proof expression labeling during Psi 05 contract checking (read: `:10872-10878`).
- Read `ProofOnlyClassification::is_proof_only` / `reason` (`evidence_and_inspection.rs:196-202`, read): keys `HashMap` by `symbol.arena_index()` only; symbol generation is not part of the key (digest `area_12.md:40`, read).
- Read `classify` (`evidence_and_inspection.rs:463-472`, read): `index_by_symbol` maps `definition.symbol.arena_index()` to definition index (read); digest generation-ignore claim at `:468` refers to this map (read).
- Read `is_proof_value_state` / `is_proof_signature` arms at `:391-393` and `:419-421` (read): `continue` when `proof_only_mention` is absent — conservative skip, not an acceptance widen (read); cited as supporting context only.
- Caller search `is_proof_only\(` in `omega-rust` (read): production uses in `validation/src/proof_contracts_core.rs` (`:393`, `:7655`, `:7769`), `validation/src/machine_calls_core.rs` (`:6522`, `:6840`), `validation/src/contract_entailment.rs` (`:16032`), and `omega/.../data_layout_queries/builder.rs:86` (read: grep listing, not every callsite body).
- Wrong program (plain words): two distinct invalid proof struct literals for the same type name (different missing case or field shapes) receive the same `invalid-constructor:<name>` label in contract proof rendering, so downstream proof text cannot distinguish them; separately, a data symbol reused at the same arena index with a different generation can inherit a stale proof-only classification.

## A1063

- Read `collect_scopes` (`service_declarations.rs:148-165`, read): ambiguous same-trait applications set `ambiguous` and return `None` at `:164` (read), not an error.
- Read `from_typed_application` (`service_declarations.rs:383-412`, read): `let scopes = applied_receivers::collect_scopes(program, root)?` at `:400` (read) propagates that absence as `None`.
- Caller search `from_typed_application` in `omega-rust`: 4 matches — definition at `:383`, calls at `:380`, `:1302`, `:1612` (read).
- Read `PreparedServiceCatalog` retain path (`service_declarations.rs:1297-1304`, `:1550-1584`, read): `projection::from_typed_application(...)` may be `None`; `retain` still `insert`s a `ServiceRecord` at `:1584` with `declaration: None` when projection is absent (read: `:1567-1583` only fills declaration when `projected` is `Some`).
- Read `receiver_applications` (`service_declarations.rs:167-191`, read): at `:183-184` and `:189-190` (read) missing portable parameter identity or lifetime arguments returns `Some(Vec::new())`, producing a surface with zero receiver applications without error.
- Read `record` (`service_declarations.rs:1413-1425`, read): duplicate `ServiceOrigin` yields `Handle::default()` at `:1421-1422` (read); `lookup` at `:1427-1429` then fails to resolve a surface (read).
- Read identity interning with `unwrap_or_default()` (`service_declarations.rs:630-635`, `:645-650`, `:946-949`, read): failed `type_identity_text` becomes empty string identities that can collide (read).
- Read `sealed_accessor` (`service_declarations.rs:1495-1512`, read): missing sealed row returns `Ok(None)` at `:1505` (read), same outward shape as “not sealed”.
- Read sealed machine census (`service_declarations.rs:1646-1650`, read): `authenticate(...).map(|d| d.filter(|d| context.contains(...)))` turns out-of-scope machines into `Ok(Some(None))` (read), not a scoped error.
- Read boundary ABI census loops (`service_declarations.rs:1761-1767`, `:1890-1909`, read): missing catalog declaration or missing portable application uses `continue`, dropping ABI rows silently (read).
- Read `owned_requirement_clause` (`service_declarations.rs:46-69`, read): ambiguous conformance parent match returns `None` at `:68` (read).
- Wrong program (plain words): a boundary trait has two incompatible parent instantiations under one receiver, or portable identities are missing, so the service catalog stores an empty or duplicate-key record and provider planning sees no service surface while typed-tree compilation completes.

## A1064

- Read `checks_authored_selection` (`typed-trees/src/lib.rs:1561-1576`, full function, read): returns true when the occurrence is not excluded and, if `product_declaration_frontier` is set, `symbols.source_file(selection.source_span())` is `None` **or** the file is toolchain/build (`:1568-1575`, read: `is_none_or` at `:1572`).
- Read `ProgramIdentity` (`lib.rs:1325-1345`, read): `PartialEq::eq` always returns `true` at `:1341-1343` regardless of the inner `u64` (read); default still assigns distinct ids at `:1336-1337` (read).
- Caller search `checks_authored_selection` in `omega-rust`: 4 matches — definition, `declarations_core.rs:127`, `frozen_program_and_queries.rs:2087`, `source_compilation.rs:231` (read).
- Read `declarations_core.rs:126-130` (read): public-interface visibility diagnostics run when `checks_authored_selection` is true.
- Read `source_compilation.rs:227-231` (read): reference/selection closure for compilation includes selections that pass `checks_authored_selection`.
- Read `frozen_program_and_queries.rs:2084-2088` (read): selections that fail `checks_authored_selection` are added to descriptor-selection exclusions; selections with no source file are **not** excluded via that filter when `checks` returns true.
- Read `PartialEq for dyn CheckingInput` (`frozen_program_and_queries.rs:676-682`, read): compares full `typed_program()` with `TypedTrees` derived equality, so the `identity` field never distinguishes programs (read).
- Wrong program (plain words): an authored declaration selection occurrence has no resolvable source file while a product declaration frontier is active, yet it is still treated as in-scope for visibility checking and for compiling reference/selection closure; two frozen checking inputs that differ only in `ProgramIdentity` counter compare equal on the typed tree component.

## What I did not read

- Full bodies of every `AppliedTraitScope::visit` consumer beyond the cited samples (`contract_entailment.rs`, `monomorphization.rs`, `traits.rs`, etc.).
- Entire `capture_static_environment` `Capture::operand` implementation below the binding loop (`product_and_trait_declarations.rs:574+` only skimmed for return paths).
- Provider-planning stages after `PreparedServiceCatalog::build` that might error on empty `declaration` (omega `provider-planning` crate not opened).
- `constructor_values::render` callers beyond `render_proof_constructor_value` (`:1809` read not opened).
- Git commit `457adf4a11f` file snapshots for line drift (cited lines matched at read on current checkout).

SLICE COMPLETE
