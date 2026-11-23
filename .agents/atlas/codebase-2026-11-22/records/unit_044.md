# Record: unit_044 (area 07-psi-semantics)

Root `S/` = `source-rust/subsystems/compiler/psi/semantics/`. Checkout `4f3c255d`. Everything below was read unless marked.

## A. `S/src/access/*` and `S/src/body_services.rs`, `S/src/bytes*` (2,869 lines, read)

### access.rs (11 lines)
Module list only (`access.rs:6-11`): access_lifecycles, access_ram_correspondence, access_ram_origins, access_view_plans, access_view_referent, accessor_scalar_boundaries. No logic.

### access_lifecycles.rs (583)
- Purpose: checks the module's table of "access lifecycle plans" (how a placed storage extent is initialized, viewed, moved out or relocated), each tied to a boundary machine declaration.
- Shape: `validate_access_lifecycle_plans(module)` `:15`; `validate_access_view_content` `:302`; private `validate_image_fields` `:364`, `validate_storage` `:443`. Re-exports `AccessResultContract`, `access_result_contract`, `access_view_reference_slot` (`:8-10`).
- Inputs/outputs: reads `module.interfaces.boundary_machines`, `module.access.access_lifecycle_plans`, `module.access.access_view_plans`, `module.types.structural_types/structural_domains`, `module.identities`. Returns `Result<(), &'static str>`; mutates nothing.
- How it decides: keyed on ids and identity text. Plans must be strictly ordered by `(boundary id, target name string)` (`:35-42`). Each declared placement primitive must have a `CompilerPrimitive` plan (`:16-31`). Boundary checks compare structural type ids, access kinds and qualification (domain id) sets (`:77-239`). Identity text must be non-empty (`:65-73`). Equal `(policy_identity, schema_identity, target name)` plans must agree on schema, image fields, storage and placement (`:272-297`). Target is compared by name string (`target.target.as_str()`, `:37,277`).
- Cases by shape: `match plan.operation` handles exactly Relocate (4 params: run, destination, 2 profiles `:161`), View (2 params `:189`), Initialize (2 or 3 params `:203`), MoveOut (1 or 2 params `:226`). Owner is `Extent` or `Run` only (`:454-510`). `validate_access_view_content` accepts only primitive scalar, fixed array, record whose fields are scalar (non-address integer), IEEE float, or structural; other shapes refused (`:335-357`). `validate_storage`: run count must be unsigned 64-bit non-address integer (`:488-496`).
- Same job elsewhere: `access_result_contract` (result_contract.rs:67) is called from here (`:244`) and from access_ram_origins.rs:253. `validate_access_view_plans` (access_view_plans.rs:374) does a parallel cross-check of view plans.
- Unused/one-caller: not grepped for the `pub fn`s individually (suspected used from `S/src/lib.rs`, pipeline stages).
- Stated limits: none quoted; error strings state refusals ("nonresident access View requires total scalar decoding evidence" `:311`).
- Fail-open: `placement image` check at `:434` compares `(size != 0)` with membership; no fail-open seen. `target: None` skips layout-target validation (`:43`): traced, a plan with no target is a target-neutral plan. No other fail-open seen.

### result_contract.rs (481)
- Purpose: derives, and checks, the exact shape of the result sum type a lifecycle boundary must return.
- Shape: enum `AccessResultContract<'module>` `:10` (View/Initialize/MoveOut/Relocate); `access_view_reference_slot` `:40`; `access_result_contract` `:67`; helpers `record_pair` `:374`, `failure_cases` `:390`, `sum_cases` `:425`, `structural_type` `:438`, `require_type` `:450`, `storage_qualifications` `:460`.
- Decides on: structural type ids, field counts, qualification sets selected at a path (`StructuralPathQualification::select`). The result must be `Affine` with no root qualifications (`:77-81`).
- Cases by shape: View: exactly 2 cases Ready/Rejected, Ready one reference field (`:85-90`); Initialize/Relocate: exactly Ready/Rejected; MoveOut: exactly one case (`:352`); failure sum has exactly 3 cases, first two carry two u64 fields, third empty (`:394-421`). `access_view_reference_slot` returns `None` unless 2 cases and one field (`:48-51`).
- Fail-open: `access_view_reference_slot` returns `Option`; callers treat `None` as refusal (inferred, not traced). None seen otherwise.

### access_ram_correspondence.rs (219)
- Purpose: matches a placed-access view plan against the real byte layout of the ordinary source record, so RAM reads and writes of fields land at the same offsets and widths.
- Shape: `access_reference_correspondence` `:16`, `access_ram_correspondence` `:47`, `access_ram_plan_correspondence` `:70`; re-exports `AccessRamCorrespondence`, `AccessRamFieldCorrespondence` (`:45`).
- Decides on: field identity ids; unique retained plan per carrier (`:59-61`); byte offset, transfer width and bit offsets equality (`:123-128, :170-184`). Validates a resource profile with one exclusive read-write region over the whole source (`:195-211`).
- Cases by shape: each field must be a scalar leaf (or a literal-length non-zero fixed array of scalars, `:140-148`); descriptor logical extent must be one fragment (`:164-169`). Inaccessible fields are skipped (`:107-109`).
- Fail-open: skipping `Inaccessible` fields (`:107`): traced to the plan's access entry; such a field yields no correspondence row, not an accepted access. None else seen.

### access_ram_origins.rs (363)
- Purpose: for a boundary call that reads or writes a placed field, finds which formation sites (reference-establishing operations or provider calls, or the entry's private inputs) can have produced the argument.
- Shape: `AccessRamFormation` `:25`, `AccessRamOrigin` `:33`, `AccessRamOriginAnalysis` `:52`; `boundary_formations` `:84`; private `resolve` `:187`, `resolve_origins` `:199`.
- Decides on: boundary id (unique plan field operation `:94-105`), only `Read`/`Write` kinds (`:106-111`), structural window projection types (`:138-160`), carrier equality with `plan.carrier` (`:181-185`). Cycle check by a `pending` query list (`:213-216`). Callback-body machines refused (`:297-299`). Entry machine uses `private_entry_roots` (`:300-317`).
- Cases by shape: `[argument] = arguments` only (`:91`); origin kinds `AccessFormation` with `EstablishAccessReference` or `BoundaryCall` only (`:236-262`); incoming slot/contents only (`:276`); everything else `_ => return None` (`:357`).
- Same job elsewhere: descriptor-origin machinery lives in `descriptors::descriptor_results` (outside unit; suspected).
- Fail-open: every `None` is a refusal here (returns `Option<BTreeSet>`); the caller treating `None` as acceptance is not traced (open).

### access_view_plans.rs (519) and input_order_tests.rs (62)
- Purpose: validates the catalog of access view plans (carrier record, accessor leaves, boundary signatures, placement identity) and the roster of view inputs.
- Shape: `access_field_leaf` `:12`, `is_access_projection_shape` `:84`, `validate_access_view_catalog` `:106`, `validate_access_view_plans` `:374`, `contains_access_carrier` `:448`, `contains_access_carrier_in_catalog` `:456`; `pub use AccessViewPlanError` `:10`.
- Decides on: ids, structural shape, identity text compare (`canonical_access_view_identity` result vs stored text `:172-178`), field name match built with `format!("#{identity}")` string (`:220-226`, a spelling compare of member identity), operation name string `"read"/"take"/"write"` against `operation_names()` (`:297-303`). Input rows must be strictly ordered by `(machine, runtime_position)` (`:388-396`).
- Cases by shape: accessor leaf must be a record with Accessor storage, no fields, byte alignment 1 (`:282-287`); arrays only literal equal non-zero length (`:44`); boundary signature shapes by kind (`:318-349`).
- Fail-open: `contains_access_carrier_in_catalog` returns `true` when a declaration is missing (`:473-475`): traced, true means "refuse more" for callers (used at access_ram_correspondence.rs:78 to refuse). Not fail-open for acceptance.
- Tests: input_order_tests.rs has 2 tests, order rule only; comment says rows name absent machines "so ordered rows still fail later" (`:37`).
- Unused: `is_access_projection_shape` is `pub(crate)`; not grepped separately.

### access_view_referent.rs (140)
- Purpose: validates a view referent (type, path, qualification list) and resolves a view input row to its machine parameter.
- Shape: `AccessViewReferentError` `:10`, `validate_access_view_referent` `:18`, `access_view_input_parameter` `:100`; re-exports `AccessViewInputError`.
- Decides on: sorted qualification ids (`:24`), unique declaration (`:48-53`), identity ids equal to row ids (`:111,121,133`), access not `Owned` (`:136`).
- Greps: `validate_access_view_referent|...` combined count in the 6-name grep gave 1 file for referent (`access_view_referent.rs:1`, i.e. only its own definition: suspected no external caller; `rg` count mode over `source-rust`).

### accessor_scalar_boundaries.rs (192)
- Purpose: for a placed-scalar accessor boundary, derives its demanded byte offset, width, access and scalar type; also projects a repeated-region leaf.
- Shape: `access_region_projection` `:11`, `accessor_scalar_boundary_demand` `:67`; re-export `AccessorScalarBoundaryDemand`.
- Decides on: unique plan field operation for the boundary id (`:76-85`); parameter order against fixed `[Structural]` or `[Structural, Scalar]` (`:115-122, :134`); descriptor xor region (`:153-169`).
- Used: `accessor_scalar_boundary_demand` appears in 2 files per the combined grep (interpreter `instructions/structural.rs:2`, plus own file); `access_region_projection` own file plus callers in the 19-file hit list (exact split not counted).

### body_services.rs (222)
- Purpose: computes for every machine the set of services it can reach, composing known call edges and using a callable value's contract envelope when the callee is unknown.
- Shape: `BodyServiceReaches` `:12` (`get` `:17`), `body_service_reaches(module)` `:26`.
- Inputs: `module.machines`, `module.interfaces.services` and `boundary_machines`, `module.execution.callables.contracts`, `terminal_callable_values(module)`, `UnoptimizedMachineBehavior::capture`. Output: sorted rows per machine, fixpoint by worklist (`:191-209`).
- Decides on: ids in sorted tables (binary search); parent-closed rows (`:94-109`); inheritance cycle by colors (`:71-93`). Opaque call must have a retained boundary whose formal telescope matches the actuals (`:144-167`).
- Greps: `body_service_reaches` 5 occurrences: own file 1, `callable_values/tests.rs` 4. No non-test caller seen in `source-rust` (grep count).
- Stated limits: doc comment `:1-2` "Derive conservative body reach independently of a named machine's authored ceiling."
- Fail-open: none seen. Unknown targets union the contract's full reach.

### bytes.rs (15) and byte_carriers.rs (62)
- bytes.rs: module list (`mod fixed_arrays`, `operation_bounds`, `byte_carriers`, `byte_sequence_concatenation`, `byte_sequence_predicates`). byte_carriers.rs: `byte_carrier_scalar_layout` `:34` returns scalar leaf offsets/size for a record whose fields are all non-erased scalars with whole-byte width (`:18-32,:43-46`). Only `Integer` with bits%8==0, `Boolean`, bounded integers (`:19-29`). Refuses (returns `None`) anything else. Combined grep: 1 occurrence (own file), so no outside caller of `byte_carrier_scalar_layout` found.

## B. `S/proofs/mathematical/*` (kernel; non-test 9,165 lines read, tests.rs skimmed)

### mathematical.rs (46)
Crate root. `#![forbid(unsafe_code)]` `:3`. Re-exports the public API `:19-46`.

### term.rs (857)
- Purpose: syntax of the kernel: universe `Level`, `Sort::{Type,Strict}`, `Term` (30 variants `:376-479`), `TermArena` with per-node free-binding summary.
- Shape: `Level` `:6` with hand-written non-recursive Clone/Eq/Hash/Debug/Drop (`:107-278`); `levels_equal` via normal forms `:331`; `Term` `:376`; `term_layout!` macro gives children and binder counts `:527-610`; `TermArena::insert` `:645`, `required_bindings` `:665`, `contains_free_variable` `:678`, `structurally_equal(_shifted)` `:747,755`.
- Decides: level equality is on normal form (constant + per-parameter offset) `:285-333`; `Level::successor` returns `None` on u32 overflow `:44`.
- Notes: `MAX_SUBTERMS = 4` `:481`. `TermArena` equality compares only `terms`, not the binding summary (`:629-633`). `Term::Dummy` is the default.

### substitution.rs (219)
`shift` `:15`, `substitute` `:23`, `instantiate_levels` `:28`, `universe_parameter_arity` `:39`; one iterative `remap` `:155` memoized by `(term, scope)` with a `HashMap`. Skips subtrees whose recorded binding need is within scope (`:84-89`). `expect` panics on overflow (`:97,108`). Test-only counter `REMAPPED_NODES` (`:6`).

### conversion.rs (241)
- `Budget` `:17` (step counter, `StepCeiling` on exhaustion), `weak_head_normalize(_retaining)` `:45,54` handles beta, fst/snd, caseTwo, J, W induction, squash and box elimination (`:100-173`); `convertible(_retaining)` `:218,231` delegate to `typing::convert_retaining`.
- Retention callback `retained(index)` stops unfolding a declaration (`:77`). Default retention is none (`:51`).
- Greps: outside the crate, `weak_head_normalize_retaining|convertible_retaining|universe_parameter_arity|identity_substitution|indexed_correctness` appear in `psi-to-native/.../final_image.rs` (4) and `bounded.rs` (4) only (count over `source-rust`, mathematical dir excluded).

### signature.rs (196)
`Declaration` `:9` (`level_arity`, `ty`, `body: Option`; assumption = no body), `Signature` `:38`, `check_signature(_with_attribution)` `:80,89` checks each declaration in order against the prefix (self or forward constants fail as `UnknownDeclaration`), `assumption_closure` `:176`, `judgment_assumption_closure` `:185` via a term walk that does not open declaration bodies until reached by closure.

### theorems.rs (58)
One declaration `identity_substitution` `:5` (transport along `Id`, level arity 2). Uses scheme_dsl.

### scheme_dsl.rs (335)
`Syntax` enum `:5`, `build` `:52` maps named variables to de Bruijn indices, `panic!("unbound scheme variable {name}")` `:62` on unknown name. Builders `v, pi, lam, app, ...`. `define`, `assume` `pub(crate)` `:31,44`.

### indexed.rs (523)
Declarations of the indexed W-family scheme: `INDEXED_AT/W/PACK/SUP/IND` = 0..4 `:14-18`, `indexed_scheme` `:429` (5 declarations, all definitions: no assumptions), `indexed_correctness` `:425`, `IndexedFamily` `:440` with spine helpers. Level params: 3, plus a 4th motive level for `IND` (`:339`).

### quotient.rs (1,297)
- 19 declarations `QUOTIENT_IS_SET..QUOTIENT_UNIQUE` = 0..18 (`:930-948`), `quotient_scheme` `:904`, `QuotientFamily` `:1033`, `EquivalenceProofs` `:1021`.
- Assumed (axiom) declarations built with `assume`: QUOTIENT `:47`, PROJECT `:52`, SET `:57`, SOUND `:65`, EFFECTIVE `:113`, ELIM `:199`, BETA `:223`. The rest are definitions (`is_set`, `transport`, `id_trans`, `transport_const`, `lift`, `id_sym`, `id_cancel`, `prop_is_set`, `box_prop`, `ind_prop`, `coverage`, `unique`).
- Stated limit/reason: comment at `:68-73` says effectivity takes the propositional/equivalence proofs as hypotheses because without them a constant relation "would let effective(x, x, refl) prove Id Two zero one".
- Same job elsewhere: nothing seen; the spine helper `QuotientFamily::spine` is separate from `IndexedFamily::spine` (indexed.rs:450), a parallel but different shape.

### certificate.rs (1,396)
- `MathematicalCertificate` `:623` (signature, level_arity, context, term, expected). `verify_mathematical_certificate` `:658` copies the certificate into the caller's arena (`share_certificate_in_place`, rejects handles that are stale, future or cyclic `:62-86`) then `check_signature`, `infer_sort` each context binding, `check_type`. Batch `verify_mathematical_certificates` `:16` imports each certificate into a fresh arena with syntax sharing (`by_syntax: HashMap<Term, TermHandle>` `:96`) and reuses a checked signature when declarations are equal (`:28-33`); error carries the certificate `ordinal`. `certificate_assumption_closure` `:681`.
- Tests inline (`:213-611`, `:695-1396`): exercise budgets, import, stack depth, tampering, level arity, assumption closure.

### typing.rs (3,997)
- Context `:38` (bindings, level_arity, signature), `CoreError` `:106` (34 variants), `infer_sort` `:210`, `infer_type` `:237`, `check_type` `:3837`, `convert_retaining` `:343`. All judgments run in `mod judgments` as an explicit-stack machine (`drive` `:2354`, `Pending` enum `:2025`, `Operation` `:2260`, `comparison` submodule `:536-1978`), so deep terms do not use host stack. Inferred types are cached per `(binding-prefix scope, term handle)` (`InferredTypes` `:455`).
- Decides: conversion at a shared type; if the shared type's sort is `Strict` then any two well-formed terms are convertible (proof irrelevance, `:1412-1415`; header comment `:7-8`). Otherwise structural equality, then weak-head normalize, then per-constructor comparison (`enter_heads` `:865`). Eta for lambda and pair by unfolding to application and projection (`:1321-1394`). No eta for `Sup` (test name `w_types_convert_componentwise_and_sup_has_no_eta`, tests.rs:1014).
- Cases by shape: universe rules: Pi sort follows codomain strictness (`:3261`); Sigma strict only if both strict (`:3265`); squash domain must be Type, box domain Strict (`:3275-3284`); identity domain must be non-strict (`:3694`); W carrier and children codomain non-strict (`:3627, 3150`); motives for caseTwo/J/IndW must land in a Type sort; Box motive may land in any sort (`:3109`).
- Fail-open: `value.require_*` use `unreachable!` on wrong continuation kind (`:2290,2299,2308`): internal invariant, not input-reachable (inferred). `Compared::Check` sets `Checked` only when `equal` true (`:3219-3224`). `CheckLambdaDomain` skips re-forming the annotation if it is structurally equal to the expected domain (`:3778-3781`; comment `:3775-3777` says the expected domain already formed). Traced.
- Public macro: `argument_error_pair_pattern!` `:3848` (used in bounded test indexed_relations.rs:541).
- Tests inline `:3854-3996` (3 tests: no-rebuild of projection types, exact binding scope for shared variables, failure not cached).

### tests.rs (5,282) (SKIMMED ONLY: function names via grep)
88 `#[test]` (Grep count). Modules by line: conversion_judgments `:1`, assumption_closure `:240`, identity_and_w_types `:314`, level_worklists `:1070`, levels_and_declarations `:1218`, normalization_worklists `:1496`, schemes_and_quotients `:1657` (indexed_schemes `:1658`, quotient_derived `:1877`, quotient_receiver_policies `:2190`, quotient_schemes `:2622`), spine_chains `:2909`, strict_layer `:3533`, substitution_worklists `:4146`, type_checking `:4391`; shared helper builders `:4757-5282`. I did not read bodies. Claims about this file are from names only.

## C. `S/proofs/bounded/tests/*` (4,637 lines, read)

All are integration tests of crate `proof-admission` (re-exports the kernel), run as one binary via `suite.rs` (`:5-25`, 20 `mod` lines; `common`, `bitwise_sibling_premises`, `contradiction_elimination`, `equality_symmetry`, `exact_multiply_result_bound`, `indexed_derivation`, `indexed_levels`, `indexed_mutual`, `indexed_nested` are in the suite but outside my unit).
- suite.rs (25): module list.
- source_checked_authority.rs (31): `EvidenceRoute::SourceChecked` on a general goal (`Truth` or `Falsehood`) gives `EvidenceError::SourceCheckedOutsideOperation` (`:19-29`).
- integer_order_weakening.rs (60): equality of Boolean or of differently sized integers cannot be weakened to `<=` via `IntegerOrderWeakening` (`:39-60`).
- predicate_conversion.rs (74): `PredicateDenotation` rule rejects wrong polarity/identity/type and unproved child; deep (65 nested not) term hits resource limit (`:23-74`).
- theorem_certificate_admission.rs (264): `a==b requires -> b==a` via `EqualitySymmetry`; checks decision `Judged(receipt{declarations:3, assumption_closure:3, context_depth:1, arena_slots:36})` (`:120-128`, a pinned arena-slot count of 36 is a brittle value) and forged goal/proof/closure rejection at the kernel.
- indexed_vector.rs (299): vector family index discipline; wrong indices give `TypeMismatch`.
- indexed_relations.rs (863): squash/box/iindW refusals; family cannot name itself (`UnknownDeclaration`); negative embedding of a completed family allowed.
- multiple_remainder.rs (456): exhaustive 8-bit check of the provisional rule `IntegerMultipleRemainder` (divisor nonzero, not signed -1), near-miss refusals.
- nonconstructive_existence.rs (605): `transfer` is derived with empty assumption closure; extraction of relevant data from `Squash` needs the `choice` assumption; refusals `SquashTargetNotStrict`, `NotAPair`.
- predicate_denotation.rs (482): IEEE comparisons stay comparisons (no NaN-assuming `Truth`), Boolean expression truth preserved under all assignments (624 expressions), resource limits (depth 64, 2500 premises).
- quotient_at_most_dividend.rs (477): provisional rule `IntegerQuotientAtMostDividend` exhaustive on i8/u8 (needs `0<=dividend`, `0<divisor`).
- remainder_within_dividend.rs (496): provisional rule `IntegerRemainderWithinDividend` (divisor `>0` or `< -1`).
- value_equality_transport.rs (505): `ValueEqualityTransport` rejects ambient equations for missing children, cycles and expansion bounded, float-field transport exact, bit identity not IEEE equality.
- Same job elsewhere: the three arithmetic-rule tests repeat helpers `carrier/value/literal/context/range/assumption` (multiple_remainder.rs:28-66, quotient_at_most_dividend.rs:28-120, remainder_within_dividend.rs:28-166), not shared via `common`.
- Stated limits: "provisional rule" comments cite `wiki/spec/proofs/foundation.md` ("Admitting an arithmetic rule") (multiple_remainder.rs:1-2).

## Greps run
- `rg -c body_service_reaches source-rust` (Grep count): 5 occurrences, 2 files, only tests outside own file.
- Combined name grep for 6 access/byte functions: 30 occurrences, 19 files (includes common words; not split per name).
- Combined kernel-name grep outside `proofs/mathematical`: 8 occurrences, 2 files.

## Coverage
- Read in full: 16,671 lines. All src files (2,869), all non-test mathematical files (9,165: certificate 1,396; conversion 241; indexed 523; mathematical 46; quotient 1,297; scheme_dsl 335; signature 196; substitution 219; term 857; theorems 58; typing 3,997), and all 13 bounded test files (4,637).
- Skimmed: `proofs/mathematical/tests.rs` lines 1-5,282 (5,282 lines; function and module names only via Grep, not the bodies).
- Not read: the bodies of `proofs/mathematical/tests.rs` lines 1-5,282.

## Five things a maintainer most needs to know
1. The kernel's `typing.rs` is one explicit-stack machine (`drive` `typing.rs:2354`); infer, check and convert all share it and a per-scope inferred-type cache (`typing.rs:455`). Strict proof irrelevance is decided only on the shared type's sort being `Strict` (`typing.rs:1412`).
2. Lifecycle validation (`access_lifecycles.rs`) fixes the exact parameter counts, sum case counts and qualification sets per operation; a plan with a different arity is refused, not adapted.
3. `result_contract.rs` and `access_view_plans.rs` compare many things as structural type ids but also by text: target names (`access_lifecycles.rs:37,277`), identity strings (`access_view_plans.rs:172-178`), `format!("#{identity}")` member names (`:220-226`) and operation names `"read"/"take"/"write"` (`:297-303`).
4. Quotient scheme has seven axioms (`quotient.rs:47-223`); every certificate that cites them carries them in its assumption closure. Indexed scheme and `transfer` are definitions only (no closure).
5. `body_service_reaches` has no non-test caller in `source-rust` by my grep (5 hits: 1 own file, 4 `callable_values/tests.rs`).

## What I am unsure about
- The 88 tests in `proofs/mathematical/tests.rs` were not read; nothing here claims what they assert beyond their names.
- Per-function external call counts for the access functions were only grepped as one combined pattern (30 hits, 19 files); I did not split them per name.
- Whether callers of `AccessRamOriginAnalysis::boundary_formations` treat `None` as refusal was not traced (open).
