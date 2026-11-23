Built from `crate-map/04-source-stages-06-07.md` and records `unit_063` (stage 06 half), `unit_064`, `unit_065`, `unit_066`, `unit_066_gap_030`, `unit_066_gap_031`, `unit_066_gap_032`, `unit_067`, `unit_068`, `unit_069` (stage 07 half), `unit_069_gap_033` (11 records), at commit `4f3c255d05599c11b1ea8ac668635f15f2a6eb86`.

# Digest: 04-source-stages-06-07 (the lowered-form checker and Psi publication)

`P06` = `source-rust/subsystems/compiler/source-to-psi/pipeline/06_lowered-program-to-checked-program`.
`P07` = `source-rust/subsystems/compiler/source-to-psi/pipeline/07_checked-program-to-psi`.
`REP` = `source-rust/subsystems/compiler/source-to-psi/representations`.
Paths without a prefix are under `P06/src/` unless the entry names `P07`. Claims marked "(checked)" were opened by me
for section 10; everything else is the record's claim, with the record's own "suspected", "inferred" and "not traced"
kept.

## 1. What this area does

This area makes the compiler's only accept/refuse decision and then publishes what was accepted. Stage 06 (the
lowered-form checker) takes the Terminal Psi tables and obligations that stage 05 wrote, checks that the obligation
rows are exactly the ones the lowered program owes, works out what every storage place holds at each use, follows
loans and invariant windows, asks one prover for a certificate for each proof obligation and has the proof kernel
(`proof-admission`) re-check it, and returns one verdict per compilation form. Stage 07 takes an accepted judgment
together with the exact typed source it came from, publishes the canonical Terminal Psi artifact, and attaches the
records the source owns: Build bindings and requests, source-selection questions, application roots, provider
demand. Most of the volume is tests: about 1,357 `#[test]` in stage 06 (crate map) and 50 in stage 07.

**Stage 06, `lowered-program-to-checked-program`** (157 files, 114,553 lines, `crates.tsv`). In:
`CompilationForPublication` (`REP/lowered-program/compilation.rs:49`), i.e. a `TerminalCompilation`
(`REP/lowered-program/lowered_program.rs:18`) and its `Elaboration` (`REP/lowered-program/program.rs:188`). Out:
`RouteVerdict` (`verdict.rs:82`: `Accepted`, `AcceptedPrefix`, `Rejected{formations, violations, unproved,
unestablished, conflicts, exceeded}`, `Incomplete`, `FallBack`) and `CheckedPublication`
(`REP/checked-program/checked_program.rs:84`). The source driver calls `judge_publication` (`publication.rs:19`),
which calls `judge_compilation` (`verdict.rs:1537`), accepts only if every form is `Accepted` (`publication.rs:83-165`),
and re-judges after the one optimization pass if one was selected (`publication.rs:24-47`). The work is spread over
these groups:

- **Verdict composition** (`verdict.rs`, 2,138 lines). `examine` (`:950`) runs the structural pre-checks
  (`generic_storage::declared`, `nominal_calls::validate`, `static_reach::validate`, `owned_cleanup::materialize`,
  `:994-997`), the reference-end planning (`:1004-1071`) and `discharge_inner` (`:1138`). `conclude` (`:1159`) turns
  per-obligation results into the verdict, keyed on `RefusalReason` (`:1206-1219`), then runs `ceilings::judge`
  (`:1363`), `external_providers::publish` (`:1400`) and `static_contracts::publish` (`:1403`).
- **Per-machine driver** (`lib.rs`, 2,750 lines). `discharge_inner` (`:435`) runs machines in parallel through
  `machine_batch::check` (`machine_batch.rs:11`, called at `lib.rs:544`); `check_prepared_machine` (`:733`) states each
  goal (`stated_goal`, `:2153`), and `decide` (`:2304`) asks `prove::Prover` for a certificate and `verify` (`:2566`)
  has the kernel accept it. Hypotheses are the knowledge rows whose `available_from` dominates the use
  (`hypotheses`, `:2515-2553`).
- **Owed rows** (`minted.rs`, 1,461 lines). `follow_the_lowered_form` (`:722`) recomputes the obligations, invocation,
  edge and value rows each machine owes and refuses tables that differ (`claim`, `:469-507`; called from
  `lib.rs:1469-1472`).
- **Storage** (`places.rs`, 9,440 lines, of which `:8283-9440` are tests, plus `places/`). `Storage::new` (`:1071`)
  derives the write log (`places/log.rs:217`, checked against the tables by `log_is_stated`, `log.rs:704`);
  `Storage::reaching` (`:3583`) is the one backward walk that answers what a leaf, case or window holds; `judged`
  (`:4883`) is the initialization judgment; `Storage::loans` (`places/loans.rs:2898`) recomputes every `Loan` row and
  judges overlap, parent access and retained-loan use.
- **Prover** (`prove.rs`, 5,318 lines, plus `prove/`). `Prover::prove` (`:1444`) builds `ProofNode` certificates; it
  "never decides" (`prove.rs:2-5`). `prove/reach.rs` classifies a failed goal as `NoRuleForGoalForm` (reported
  `Incomplete`, still refused) rather than `NoDerivation` (rejected) (`lib.rs:2470-2474`).
- **Knowledge and inference.** `known.rs` (equality transport at one point), `graph.rs` (dominance, `dominates`
  `:253`), `joins.rs` (inferred join predicates, `infer` `:371`, `check` `:1035`), `arrivals.rs` (authored loop-head
  invariants, `admit` `:51`), `entry_inputs.rs` (inferred `requires` for private helpers, `infer` `:48`),
  `result_summaries.rs` (inferred `ensures`, `infer` `:84`), `formation.rs` (every partial arithmetic in a contract
  clause must be proved total first; owns `ReplayScope`, `prepare_replay` `:33`).
- **Calls and interfaces.** `call_contracts.rs` (the public contract of every lowered invocation, `invocation`
  `:416`), `crashes.rs` (`coverage_goal` `:57`), `ceilings.rs` (five permission axes, `judge_axes` `:947`),
  `operational.rs` (suspend/block, `judge` `:154`), `static_contracts.rs`, `static_reach.rs`, `external_providers.rs`,
  `refinement.rs`, `rankings.rs` (`admit` `:127`), `reuse.rs` (carried certificates for generic forms, `Carry`
  `:1511`).
- **Publication helpers.** `publication/optimization.rs` holds the only optimization pass,
  `SparseConditionalConstantPropagation` (`:28`); `publication/advisories.rs` holds warnings; `describe.rs` writes
  refusal text.

**Stage 07, `checked-program-to-psi`** (19 files, 10,477 lines, `crates.tsv`). In:
`CheckedPublication<LoweredSourcePublication>` through `AcceptedTerminalSource::from_judgment`
(`P07/src/accepted.rs:209`), which refuses unless every accepted form kept its source correspondence
(`accept_judgment`, `accepted.rs:223-225`). Out: `ProducedTerminalProgram` (`P07/src/custody.rs:65`) and the canonical
artifact. The work happens in:

- `prepare_publication` (`accepted.rs:280`) and `PreparedTerminalPublication::publish` (`accepted.rs:40`): emit the
  module and evidence, build Build companions (`build_companions::produce`, `P07/src/build_companions.rs:66`) and the
  selection question packet (`source_selection_publication::prepare`, `P07/src/source_selection_publication.rs:95`).
- `SourcePublication::publish` (`P07/src/source_publication.rs:136`): publishes once per roster, memoized in a
  `OnceLock` (`:194`), and admits selection questions.
- `PreparedTargetSupply::prepare` / `settle` (`P07/src/provider_supply/mod.rs:163`, `:400`): walk the call graph per
  target, pick a supply by authority (Build selection > native binding > target default, `:76-80`), and reject
  requirements with no supply or several.
- `CheckedBuildReceiver` (`P07/src/build_receiver.rs:64`), `SourceInvocationSite::check`
  (`P07/src/invocation_site.rs:64`), `TerminalSourceSelections::admit` (`P07/src/source_selections.rs:122`), and the
  persisted record (`custody.rs:78`, `:119`).

## 2. How it decides

Counts are of places the records cite, not of distinct rules.

**Identity** (stage 06: 17 places; stage 07: 10).
- Stage 06: owed-row claim by exact kind and program point (`minted.rs:469-507`); loan rows matched by `(captured
  place, access, created_at, !claimed)` (`places/loans.rs:879-882`, `:971-974`, `:1210-1214`, `:2549-2552`) and lender
  (`:893`, `:985`, `:1225`, `:2556`); invocation row to lowered call by `InvocationSite`, duplicates refused
  (`call_contracts.rs:406`, `:429`); service, parameter and contract ids in ceilings (`ceilings.rs:493-513`);
  loop-head arrival rows (`arrivals.rs:66-113`); nominal-call rows ordered by `(caller, operation, argument)`
  (`nominal_calls.rs:45-55`, `:118-125`); static reach tables (`static_reach.rs:21-287`); verdict class by
  `RefusalReason` (`verdict.rs:1206-1219`); restated published clauses (`verdict.rs:2078`); proof-callable
  declarations (`verdict.rs:366-384`); carried-proof offer by `specialization` id (`machine_inputs.rs:69-76`); write
  log rows (`log.rs:741`); specialization tables against the generic's (`reuse.rs:1676-1740`); exactly one carried
  certificate per call judgment (`reuse.rs:2159-2165`); join values compared "by identity" (`joins.rs:53`); crash cause
  and site (`crashes.rs:75-99`); hypotheses sharing a `ValueId` with the goal (`connected`, `lib.rs:2360`).
- Stage 07: companion sets bound to the artifact's custody identity (`build_bindings.rs:128`, `:134`, `:141`,
  `:148`, `:259`, `:390`) and Build sites by `(machine, operation)` (`:207-216`); requirement reviews by
  `(arena_index, generation)` (`accepted.rs:237-238`, `:603-610`); selection-to-requirement matching by handle, never by
  name (`provider_supply/mod.rs:516-524`); numeric demand by boundary id (`package_review.rs:95-99`); selections by
  custody identity and sorted consent (`source_selections.rs:41`, `:105`, `:140`); selection joins by
  `(MachineId, InvocationSite)`, `(caller, ContractId, MachineId)`, `(owner, ExpressionHandle)` and `StructuralTypeId`
  (`source_selection_publication.rs:168-181`, `:193`, `:296-301`, `:954-957`); the Build receiver
  (`build_receiver.rs:70-73`); generated call sites (`invocation_site.rs:65`, `:80-89`, `:155`); demand sealed by weak
  pointer identity (`provider_supply/demand.rs:43-47`).

**Place** (stage 06: 12).
- Which writes end a contents, case or window query, by place overlap (`places.rs:877-908`, overlap at `:201`);
  every leaf under a used place must not be `Unestablished` (`places.rs:4916-4931`); case facts from dispatch edges and
  membership tests (`places.rs:3310-3401`); window consumption (`places.rs:6018`); captured regions with runtime
  indices (`places/backing.rs:203-231`); loan overlap (`places/loans.rs:205-232`); indexed store alternatives
  (`places/goal_observations.rs:75-92`); snapshot acceptance and the covering mutable actual
  (`places/contents.rs:175-203`, `:96-145`); `Storage::untouched` for `Preserved` (`places.rs:1836-1880`); contract
  roots by `PlaceId` (`formation.rs:465-477`); byte reads need the measurement of the same view taken directly before
  (`places/byte_views.rs:702`, `:788`, `:1176`).

**Value** (stage 06: 11; stage 07: 1).
- Stage 06: prover choices by proposition form and literal values (`prove.rs:134`, `:317-322`, `:531-571`); which
  equation defines a value (`known.rs:424-451`, `:481-488`, `:615-681`); order bitmasks over integer comparisons
  (`entry_inputs/integer_cases.rs:16-52`); binding a caller value to a callee formal through equal values
  (`entry_inputs.rs:1001-1031`); rank normalization (`rankings.rs:776-828`); the SCCP lattice
  (`publication/optimization.rs:59-66`, `:186`); literal byte predicates (`places.rs:5409`); literal selectors
  (`places.rs:6222-6289`); hard-coded `AsciiOnly` bound 127 and `NoNul` lower bound 1 (`places/byte_views.rs:142-145`);
  parameter widths (`prove/widths.rs:165`); "beyond the rules" by term variant (`prove/reach.rs:122-165`).
- Stage 07: supply authority is enum order, then exactly one (`provider_supply/mod.rs:76-80`, `:359-371`).

**Position** (stage 06: 14; stage 07: 1).
- Stage 06: dominance by program-point kind (`graph.rs:253-313`); hypotheses filtered by dominance
  (`lib.rs:2526-2528`); block ids must equal position + 1 (`graph.rs:55`); lowest failing machine id wins
  (`machine_batch.rs:38`, `:55-65`); root at position 0, others additional (`publication.rs:91-103`); first span match
  in `head_arrival` (`describe.rs:291-294`); first row of a kind in `owed` (`minted.rs:430-435`); reverse postorder and
  join skipping (`joins.rs:440-470`); helpers processed by remaining incoming calls (`entry_inputs.rs:61-106`); first
  `Unsupported` refusal per machine (`verdict.rs:1663-1667`); first consumption point per window (`verdict.rs:1256`);
  branch-condition exclusion by edge (`branch_warnings.rs:212-217`); verdict order closure, forms, witnesses, further
  (`verdict.rs:1620-1653`); diagnostic machine fallback `MachineId::new(1)` (`formation.rs:52-53`, `:75-80`,
  `:100-103`, `:980-984`; `callable_arrivals.rs:36`; `generic_storage.rs:10`) and probe id `ObligationId::new(1)`
  (`branch_warnings.rs:247`).
- Stage 07: forms count `1 + further` (`accepted.rs:223-225`).

**Spelling** (stage 06: 4 deciding places plus diagnostics; stage 07: 12).
- Stage 06 deciding: generic storage identity text compare (`generic_storage.rs:48-49`); type description text parse
  (`nominal_calls.rs:81-85`); non-empty identity text of an instance member (`static_contracts.rs:163-167`); member
  identity text non-empty and service identity text lookup (`static_reach.rs:225-237`, `:267-279`). Display only:
  arm labels and machine names in diagnostics (`verdict.rs:197-204`, `:707-714`, `:803-824`, `:1982-1987`) and all of
  `describe.rs`.
- Stage 07 deciding: Build protocol by four names (`build_companions.rs:105-112`); operation by requirement-name
  string (`build_companions.rs:132`, `:508-512`, `:514-560`); provider option by declaration and case names
  (`build_companions.rs:638-666`); `"Build"` receiver type (`build_receiver.rs:51-58`) and storage identity text
  (`:109-111`); `display_path(.., "::") == declaration_path` (`accepted.rs:666-673`); callable identity text
  (`invocation_site.rs:209-262`); requirement identity text (`provider_choice_inputs.rs:115-116`, `:250`); target
  canonical name and identity text (`provider_supply/demand.rs:207`, `:236`; `provider_supply/mod.rs:270`, `:279`);
  computed identity strings in the selection packet (`source_selection_publication.rs:438`, `:581`, `:612-616`,
  `:653`, `:1434`, `:1764`).
- Tests: most negative tests in both crates pin exact refusal text (records 066-069 list over 100 strings), and pick
  machines by name (`boundary_calls.rs:58`, `operational_effects.rs:35`, `indexed_contracts.rs:8-11`), by display path
  (`provider_supplies.rs:344`), or by `identity.contains(..)` (`machine_lifetimes.rs:67`, `structural_domains.rs:466-467`).

## 3. The same job in more than one place

- **Prover and kernel calls.** `prove.rs:2-5` calls it "the one prover"; it is one type with 6 production
  `Prover::new` sites in 5 files (`branch_warnings.rs:249`, `entry_inputs.rs:713`, `joins.rs:605`, `lib.rs:2464`,
  `lib.rs:2727`, `result_summaries.rs:527`) (checked; record 065 also counted `machine_batch.rs:94`, which is inside a
  test). `formation.rs:440-461` reaches the same pair through `search_certificate`/`verify_certificate`.
- **AND/OR construction.** `joins::connective` (`joins.rs:329`) and `entry_inputs::canonical_parts`
  (`entry_inputs.rs:1088-1205`) share the unit/zero rule; `canonical_parts` adds flattening and codec ordering
  (checked).
- **Knowledge-by-dominance walks.** `entry_inputs::known_at` (`:752`), `entry_inputs::derived` (`:686`),
  `joins::describe` (`:652-654`), `lib.rs::hypotheses` (`:2526-2528`).
- **Storage adapters.** `Storage::at_call` (`places.rs:1477`) and `at_cleanup_call` (`places/cleanup.rs:12`);
  `Storage::backing` (`places/backing.rs:699-750`) and `cleanup_backing` (`places/cleanup.rs:54-113`), which differ
  only in how they locate the point and in error text (checked); five `Bindings` impls with near-identical
  `field`/`extent`/`byte_predicate` bodies (`places.rs:6417`, `:6650`, `:7386`, `:7576`, `:8032`).
- **Row re-derivation.** `minted.rs` and `Storage::owed_are_minted` (`places.rs:1404`, `DataDomain` rows);
  `owned_cleanup::materialize` mints cleanup rows that `minted.rs:1001-1033` and `cleanup_log_is_stated`
  (`places/cleanup.rs:190`) re-check; `log_is_stated` (`log.rs:704`) and `cleanup_log_is_stated` do the same check
  for ordinary and cleanup writes.
- **Loans.** Cleanup conflict logic in `uses_retained_loan` (`places/loans.rs:1431-1489`) and `successor_borrows`
  (`:1950-1985`).
- **Ceilings.** The per-invocation block of `judge_axes` (`ceilings.rs:1182-1253`) and `tail_contribution`
  (`:1325`) repeat the same steps. The five axes are listed in `verdict.rs:651-661`, `:675`, `:730-740` and
  `refinement.rs:89-105`.
- **Verdict text.** The `plain_affine::Error` mapping twice (`verdict.rs:1034-1052`, `:1074-1093`); the
  `UnprovedObligation` build twice (`:1226-1242`, `:1287-1306`); the same span tail in five `reject_diagnostic` bodies
  (`:535-545`, `:590-597`, `:621-628`, `:665-672`, `:782-789`).
- **Header compares.** `external_providers.rs:89` and `:222`.
- **Prover internals.** `landing` (`prove/ranges.rs:117`) and `find_axiom_before` (`:1065`); `order_through_cast`
  (`prove.rs:2366`) and `cast_bound` (`ranges.rs:690`); order chains in `relation` (`prove.rs:2812`,
  `:3163-3313`) and `widths.rs::value_bounds` (`:249`); `mathematical_value_order` (`prove.rs:2454`) and
  `prove/mathematical_bounds.rs`; `refute*` (`prove.rs`) and `refute_ieee_relations` (`ieee_float_order.rs:145`).
- **Renaming values.** `reuse::Transport` (`reuse.rs:289`), `substitute_goal` (`lib.rs:1803`), `ScalarActuals`
  (`lib.rs:2017`); `wire/src/contract_forms.rs:108` suspected.
- **Call walks.** `nominal_calls.rs` and `minted.rs:176-231` walk the same caller/operation/argument structure.
- **Adjacency rule.** `places/byte_views.rs:702` and `collection_index_goal` (`places.rs:2726`).
- **Stage 07.** Seven identical custody-check accessors (`build_bindings.rs:128`, `:134`, `:141`, `:148`, `:259`,
  `:390`); record and sum-case field handling (`build_companions.rs:796-820`, `:828-842`); native formation capture
  twice (`:196-228`, `:562-621`); three provider-selection structs (`P07/src/provider_choice_inputs.rs:170`,
  `REP/retained-product-selections/selections.rs:15`, `REP/lowered-source-publication/provider_selections.rs:9`,
  suspected conversions of one fact); three seal styles (`provider_supply/demand.rs:24`, `source_selections.rs:16`,
  `REP/published-target-providers/publication.rs:32`); two call-graph walkers, `Elaboration::call_closure`
  (`REP/lowered-program/program.rs:406`) and `SynchronousCallGraph` (`provider_supply/mod.rs:205`), suspected;
  `reached_native_bindings` (`demand.rs:198`) and `provider_supply/package_review.rs` both call
  `materialization_for`; `check_selections` and `publish` both admit questions (`source_publication.rs:110-130`,
  `:147-160`); requirement identity text computed at `source_selection_publication.rs:575` and
  `provider_choice_inputs.rs:115`, `:133`.
- **Tests.** The `Compilation { .. }` literal 68 times across `src/tests/` (record 067) beside the shared
  `build_purpose::roster` (`build_purpose.rs:170`); 16 copies of a private `fn verdict(` in `src/tests/` (checked);
  repeated judge-publish-instantiate-invoke sequences (`array_lengths.rs:203`, `atomic_wake.rs:116`,
  `borrowed_restoration.rs:628`, `loan_regions.rs:60`, `stored_references.rs:133`, `case_payloads.rs:555`); a twelve-field
  `Signature` literal about eight times (`tests.rs:6441-6455`, `:7044-7058` and others); `Verdict`/`judged` copies in
  `P06/tests/block_values.rs:26-42`, `subjectless_match.rs:27-43`, `variant_parent_identity.rs:94-117`.

## 4. Fixed lists of shapes

Stage 06 production:
- Goal routes: `stated_goal` refuses `Claim`, `Loan`, `RestorationWindow`, `DomainDebt`, `EstablishmentDebt`,
  `WellFoundedness` (`lib.rs:2287-2297`); a goal or hypothesis naming a type-parameter value is refused
  (`lib.rs:2125-2150`); erased actuals only `Value`/`Integer`/`Boolean` and no erased proof formal
  (`lib.rs:1883-1908`); `ScalarActuals::field`/`case` refuse (`lib.rs:2066-2087`).
- Owed bounds: an explicit list of seven operations owe index/bounds obligations (`minted.rs:917-939`); a width subject
  must be a `u64` proof-static input (`minted.rs:338-349`).
- Storage: `field_shape` "a field type outside plain data" (`places.rs:1991`); root kinds `ProviderAttachment`,
  `TrivialAffineLocal` refused (`:1967-1969`); index only into literal arrays, parameter-extent arrays or element views
  (`:2253-2258`); `StructuralCaseLeafCopy` refused (`:4608-4610`); storage on a conditional edge (`:5792-5799`) or a
  dispatch edge (`:4993-4995`) refused; provider record leaves only `bool`, `f32`, `f64`, 8-64-bit non-address
  integers (`:3653-3666`); `MAXIMUM_FIELD_CROSSINGS = 256`, records and `Integer`/`Boolean` fields only (`:7077`,
  `:7096-7105`); byte-literal establishment from four operations only (`:5228-5285`).
- Loans: only `EstablishReference`, `EstablishElementView`, `EstablishMutableElementView` form persistent loans
  (`places/loans.rs:1067-1087`, checked); write-only loans refused (`:853-855`, `:1096`); exclusive representation views
  refused (`:1115-1118`).
- Write log: an explicit `OperationKind` match; others refused unless they produce nothing and have a result equation
  (`log.rs:280-649`); `CompareExchangeOnce` (`:324`) and mutable sequence construction (`:371`) refused.
- Byte views: byte element must be plain `u8` (`places/byte_views.rs:31-38`); refusals at `:378`, `:481`, `:523`.
- Prover: `prove_structural` refuses six proposition forms (`prove.rs:2055-2060`); many helpers only for fixed-carrier
  non-address integers (`prove.rs:3056`, `:3122`, `:3175`, `:3221`, `:3243`, `:3740`, `:3918`); `chain_operands` covers
  ten operations (`prove/ranges.rs:65-81`); `multiple_remainder_bound` seven term forms (`:972-981`); width goals with
  one or two operands (`prove/widths.rs:104-118`, `:193-200`) and exponent `N` or `N - 1` (`:545-553`); hashed facts only
  over leaf terms (`prove.rs:416-433`).
- Inference: integer comparisons over `Value`, `Integer`, `IntegerField`, `Application` of one fixed-width
  non-address type (`entry_inputs/integer_cases.rs:116-134`); call sites `Operation` or `InvokeNoNormal` only
  (`entry_inputs.rs:606-641`); one member correspondence per loop head (`:510-516`); `InputBindings::field`/`case`
  always refuse (`:1048`, `:1058`); join scope excludes six proposition forms (`joins.rs:199-223`) and Boolean
  operation results (`joins.rs:273-327`); result summaries only for scalar results, comparisons of whole values
  (`result_summaries.rs:51`, `:298-312`, `:330-336`).
- Formation: descriptor extent only on `ElementView`/`BorrowedView` (`formation.rs:389-402`); `case` always refuses
  (`:829`); qualified mathematical predicate inputs refused (`:1025`).
- Calls: boundary calls with structural qualifications, content guarantees, result reference regions or local root
  introductions refused (`call_contracts.rs:540-569`; `external_providers.rs:98-110`); external completion with a
  crash route refused (`call_contracts.rs:505-511`); tail calls with erased arguments refused (`:289-291`); a trapping
  primitive only `Trap` (`crashes.rs:151-157`); one invocation row per site (`:196-202`); possibly-suspending terminal
  calls refused (`operational.rs:314-335`); only `InvokeOwned` cleanup actions (`owned_cleanup.rs:170`, `:364-372`).
- Ceilings: nested structural parameter paths, non-primitive callable carriers, opaque callables without origin,
  open abstract reach under a finite ceiling, conditional callee termination (`ceilings.rs:457-458`, `:518`, `:580`,
  `:680`, `:749`, `:1190`, `:1197`, `:1213`, `:1236`, `:1344`, `:1356`, `:1368`, `:1448`).
- Generics and reuse: more than 8 `requires` clauses refused (`reuse.rs:1119-1123`); `Transport::term` five term forms
  (`:324-372`); `Carry::proof` four obligation kinds (`:1867-1899`); `IntegerCastBound` and
  `IntegerCorrelatedForbiddenRoots` not movable (`:703-706`); static progress premises refused
  (`static_contracts.rs:121-130`).
- Rankings: rank carriers fixed finite non-address integers (`rankings.rs:239-249`); rank values from constants,
  exact/saturating subtraction and descriptor length only (`:975-981`).
- Verdict: `fall_back_or_defect` handles four `(reason, site)` shapes, anything else is an error
  (`verdict.rs:2007-2071`); closures listing generic templates or conformance witnesses refused (`:974`, `:985`).
- Optimization: one pass (`publication/optimization.rs:28`, others refused `:38-43`); seven evaluated operations
  (`:186`).

Stage 07 production:
- `unplanned_demands`: five module-level demand kinds refused "no product route yet" (`provider_supply/mod.rs:563-594`,
  `:455-457`); `unplanned_boundary` refuses numeric intrinsics, non-`Callable` roles and non-`Provider` execution
  (`:599-618`).
- Native locator shapes: scalars, shared byte views, records, closed fixed arrays, sums with scalar or structural case
  fields; everything else refused (`build_companions.rs:766-843`); `protocol_action` refuses two describe operations
  (`:493-495`); operation names in three fixed lists (`:514-560`).
- `invocation_reference`: three call-site shapes (`source_selection_publication.rs:377-421`); `local_binding`: seven
  binding kinds (`:1826-1833`).
- `build_backing`: closed implementation of a boundary requirement only (`accepted.rs:682-690`); `entry()` machine
  entries only (`accepted.rs:646`); Build receiver a free machine with one original Build parameter
  (`build_receiver.rs:29`, `:36`); generated callers with one state ending in the call transition
  (`invocation_site.rs:169-188`).
- `requirement_owner` refuses boundaries with type or lifetime parameters (`provider_choice_inputs.rs:310-311`).
- Persisted programs: one schema `OMGBLD03`, and only with every Build companion list empty (`custody.rs:17`,
  `:50-60`).

## 5. Accepts when it does not know

Classes are the records' unless marked. "Fewer premises" means the code drops a premise, which can only make a later
proof fail.

Stage 06:
- `ceilings.rs:1300-1302`: an axis with an open caller ceiling is skipped. Traced (063): open callee under finite
  caller refuses first (`:1189-1200`, `:1212-1216`); unknown operational bounds refuse (`:329-335`).
- `ceilings.rs:343-348`: unknown suspension/blocking ignored when the caller's is also unknown. Stated (`:341-342`).
- `ceilings.rs:1238-1253`, `:1371-1380`: `fixed_refinement_reach` replaces an exceeding reach in installation-bound
  refinement. Open.
- `crashes.rs:77`, `:82-87`, `:92`: an unconditional published route returns `Truth`. Stated (`:11-15`).
- `branch_warnings.rs:247-272`: any failure means no warning, never acceptance. Stated (`:167-169`).
- `entry_inputs.rs:1267-1288`: an inferred conjunct that cannot form is dropped. Stated (`:1219-1222`). `:1260-1261`:
  non-`ForeignIdentity` formation errors read as "does not form". Traced. `:1234`: exhaustive disjunctions removed.
  Traced (tautologies only). `:806-808`: callable formals skipped. Stated. `:364-366`: `unreached.mark_formal`. Open.
- `entry_inputs/inferred_calls.rs:136-144`: calls not discharged by certificate are left out of the retained record.
  Traced (narrows publication, not acceptance).
- `external_providers.rs:140-144`: `.zip` of structural parameters stops at the shorter list. Record 063 classed it
  open; I traced it (checked): `contract_refinement_goals` refuses unequal `structural_parameters` lengths
  (`psi/semantics/src/calls/callable_refinement.rs:331-333`) and is called a few lines later
  (`external_providers.rs:168-169`). Corrected: traced, not fail-open. `:54-58` zero candidates: nothing to publish.
- `formation.rs:525-538`: a clause that does not bind and does not "owe" gives no premise. Stated (`:529-533`).
  `:904-919`: nothing to form returns `Ok`. Traced. `:565-569`: float-meaning validation `Ok`. Stated.
- `joins.rs:887-889`: no predicate row gives goal `Truth`. Traced. `:504-507`: unbound snapshot terms dropped.
  Stated (`:43-47`). `:602-604`: empty bounds on failure (fewer premises). `:257`, `:296`: `Err` becomes `false`.
- `known.rs:712-714`, `:744-747`: kernel errors mean no rewrite or `false`. Traced.
- `lib.rs:1761-1789`: an unread entry-snapshot part is kept unsubstituted. Stated (`:1758-1760`). `:1803-1861`:
  undecidable disjuncts dropped (only strengthens the goal). Stated. `:2640-2644`: other forms unchanged. Stated.
  `:1603`: no return edge, vacuous. Stated. `:855-876`: `TargetCondition` has no certificate; the target closure
  decides later. Stated (`:5-7`). `:1009`: `Preserved` without certificate when no overlap. Traced (an `Unsupported`
  clause returns `false`, `places.rs:1857`).
- `machine_inputs.rs:51`, `:60`, `:69`: offers naming no machine ignored. Stated (`:72-75`).
- `nominal_calls.rs:118-127`: a call input with no row and not `required` is skipped; a const-dependent type missing
  from `nominal_types` is not caught here. Open.
- `owned_cleanup.rs:518-527`: an inaccessible reference leaf gets no write row (checked: plain `continue`, no
  comment). Open.
- `minted.rs:804-808`: edge rows no terminator owns. Stated. `:587-636`: an unreachable call needs no claim frontier
  (decided in `plain_affine`). Open. `:1395`: no `domains` row means no clauses. Stated.
- `places.rs:4025-4038`: a formal arrives `Established`. Stated. `:3724-3755`: admitted provider referent
  `Established`. Stated. `:3514-3596`: window handed off whole is `Closed`. Stated and traced. `:4021`: window at
  entry `Closed`. Stated. `:5084`: `byte_transfer_qualifications_hold` returning `Ok(None)` accepted. Open.
  `:5444`, `:5477`: `Boolean(true)` from declared source predicates. Stated (`:5321-5322`). `:7699-7723`,
  `:7845-7850`: undecided terms, not verdicts. Stated.
- `places.rs:3644`: an unresolved cycle answers `Unknown` for a window query. Open (064). Extra trace (checked):
  `open_at` and `open_before` test `== Reached::Open` (`places.rs:5943`, `:5979`), so `Unknown` reads as "not open";
  `merge` lets `Open` win over every other supply except `Unestablished` (`places.rs:498-509`), so `Unknown` arises
  only when no path supplies the window. Class unchanged: open.
- `places/arrivals.rs:21-30`: out-of-range block or position gives `Ok(false)` (not borrowed). Open.
- `places/byte_payloads.rs:533-535`: borrowed carrier skipped. Stated (`:528-530`). `:570`:
  `DescriptorCallOrigin::Empty` adds nothing. Open.
- `places/byte_views.rs:149-161`: falls back to declared argument predicates. Traced to that callee only.
- `places/loans.rs:1086`: operations that are not reference formations form no loan (checked). Open: a formation kind
  missing from the match with no table row would form no loan. `:800-808`: all-owned arguments. Traced.
  `:2338-2347`: no schedule retires fewer loans. Stated. `:1529`: no competing borrow. Stated (`:211-214`).
  `:2843-2848`: no returned part. Stated. `:1322-1324`: duration-only loan. Stated.
- `places/log.rs:536-543`: `DecomposeStructuralValue` and `ByteSequenceWrite` emit no store row. Stated.
- `prove.rs:1068-1082`, `:1939-1941`: `ex_falso` from `Falsehood`. Stated; the kernel re-checks.
- `prove/reach.rs`: "beyond the rules" turns a reject into `Incomplete`, which still refuses. Stated.
- `publication/optimization.rs`: ordering compare on two Boolean facts. Open.
- `rankings.rs:562-568`: strict edges relabelled `Preserving` when the preserving graph is acyclic. Traced to the
  acyclicity test; the downstream consumer of the relabelled `ranked_scc` not read.
- `refinement.rs:157-161`: descriptors with imported evidence skip derivation. Stated at the site; whether
  `validate_descriptor_refinement_evidence` covers them is open.
- `result_summaries.rs:523-526`, `:576-578`, `:612-614`: summary dropped (written contract only). Stated.
- `reuse.rs:776`: failed carry falls back to an ordinary proof. `:507`: primitive judgments copied; the kernel
  re-decides. Traced. `:1683-1691`: extra `StorageContent` values accepted. Stated (`:1666-1670`).
- `static_contracts.rs:435-437`: open crash policy. Stated. `:89`: `signature == None` skips the parameter check.
  Open. `:284-295`: opaque rigid formal skipped. Stated. `:143-145`: no `instance`, not compared. Open.
- `static_reach.rs:311-313`: `call_reach` returns no contribution for a call with a `required_contract`, without a
  comment. Record 066 classed it open. I traced it (checked): the only caller then uses
  `envelope.reach()` (`ceilings.rs:1193-1194`), and `envelope_for` gives such a call the required contract's runtime
  declaration envelope or refuses (`ceilings.rs:266-279`); an open reach under a finite ceiling refuses
  (`:1195-1199`). Corrected: traced, not a skipped check. `:333`: no application and no summary. Inferred.
  `:56-62`: all four tables empty skips the remaining checks. Open.
- `verdict.rs:1175`: requirement-body verdicts skipped in a generic check form. Stated (`:896-905`). `:1839-1849`:
  `carried` takes the first matching requirement machine with no ambiguity check (checked). Open (the kernel re-check
  of carried certificates is not read here). `:453`: an `Err` header gets no fact. Traced. `:1261`. Stated.
  `:1455`: generic forms carry no seals and cannot be published. Stated. `:1473`: gate not traced.

Stage 07:
- `accepted.rs:554-561`: `slot_schema` `.ok()` makes any projection error look like "not a slot". Open.
  `:150-154`: no authored source. Stated. `:514-523`: provider with empty rows publishes `code: None`. Open.
- `build_bindings.rs:213-216`: companions checked only as a subset of emitted calls. Stated (`custody.rs:136-140`).
- `build_companions.rs:88-104`: a non-protocol boundary gets no row. Stated. `:276-287`: any readiness row removes an
  outside call from `unowned`. Stated (`:274-275`). `:583-591`: unselected Product source. Stated.
- `custody.rs:154-168`: `Err` applications stored without check. Traced. `:221-230`: the `Err` branch of `owners`
  handed to `published_entry::issue`. Open (068); 069 read `issue`, which makes custody `Err` when `owners` is `None`
  (`P07/src/published_entry.rs:17`, `:35`); the `Some(Err)` case is not stated.
- `invocation_site.rs:263-277`: no emitted site, only the argument role is checked. Stated and traced.
- `package_review.rs:95-101`: `Callee::Selected` not counted. Open in 068; 069 read `call_closure`, which returns
  `None` when any callee is `Selected` (`REP/lowered-program/program.rs:442`), and `None` makes the review list every
  numeric boundary (`package_review.rs`, stated `:1-3`). So traced via 069's reading, not by me.
- `provider_choice_inputs.rs:101-109`: `Ok(None)` skips the supply; settlement rejects a requirement left unsupplied
  (`provider_supply/mod.rs:424-427`). Traced.
- `provider_supply/demand.rs:219`: a supply matching no reached requirement is skipped silently. Open.
- `provider_supply/mod.rs:298-319`: no supply leaves `realization: None`; `settle` rejects it. Stated and traced.
  `:229-240`: direct compiler primitives need no provider. Stated.
- `published_entry.rs:29`: a calling request with an `Err` declaration dropped. Open.
- `source_selection_publication.rs:1002-1011`: folded calls with a missing edge or head source dropped. Open.
  `:1064-1076`: boundary calls without a source site not recorded. Traced (refused later, `:1341`). `:1607-1611`:
  `purpose()` falls back. Open.
- `source_selections.rs:63-68`: no calls or no entry, nothing to admit. Stated. `:98`: other join kinds skipped.
  Traced (fail-closed).
- `source_publication.rs:48`: no optimization record means default selections. Stated and traced.

## 6. Silent drops

Places where something the code cannot plan, resolve or lower produces no row and no diagnostic:
- `owned_cleanup.rs:518-527` (write row for an inaccessible reference leaf).
- `places/loans.rs:1086` (non-formation operations form no loan; see open item above).
- `places/log.rs:536-543` (`DecomposeStructuralValue`, `ByteSequenceWrite` store rows; stated).
- `places/byte_payloads.rs:533-535`, `:570`.
- `machine_inputs.rs:51`, `:60`, `:69` (offers or requirements naming no machine).
- `nominal_calls.rs:118-127`.
- `entry_inputs/inferred_calls.rs:136-144`; `entry_inputs.rs:1267-1288`; `joins.rs:455-470`, `:504-507`;
  `known.rs:865-873` (premise not normalized on kernel error).
- `static_reach.rs:305-313`, `:333` (no reach contribution; covered by the caller's envelope path, section 5).
- `static_contracts.rs:143-145`.
- `verdict.rs:450` (a body ending in `PendingNormalCompletion` skipped unless its header claims a checked body).
- `publication.rs:115` (incomplete unproved entries left out of a `Rejected` list; reported by the `Incomplete` path).
- Stage 07: `build_companions.rs:88-104`, `:583-591`; `provider_supply/demand.rs:219`; `published_entry.rs:29`;
  `source_selection_publication.rs:1002-1011`, `:1064-1076`; `source_selections.rs:98`; `accepted.rs:398` (direct
  compiler primitives omitted from declared boundary requirements); `provider_supply/mod.rs:229-240`.

## 7. Unused or unwired

- `reverify`, `reverify_at_call`, `reverify_specialization` (`reuse.rs:48`, `:134`, `:818`; re-exported at
  `lib.rs:97-100`): `#[cfg(any(test, feature = "test-support"))]`, no production caller (065: 22 test hits; checked:
  outside `P06/src` only a doc comment at stage 05 `lib.rs:553`).
- `prove_callable_refinement` (`refinement.rs:24`): test-support, 3 test uses.
- `judge_declared_rows` (`ceilings.rs:940-941`): test-support; its one caller (`refinement.rs:81`) is inside the
  test-support `prove_callable_refinement` (checked; 063 called it a non-test caller).
- `has_rules_for` free function (`prove.rs:236`): `cfg(test)`; the method has 1 production call (`lib.rs:2470`).
- `reference_graph` (`places/loans.rs:238`): `cfg(test)`, 8 test calls.
- `discharge_carrying`, `prepare` (`lib.rs:426`, `:1409`): test-support. `Guarantees::clause`
  (`call_contracts.rs:86`): test-only.
- `CompilationRefusal::Binding` (`verdict.rs:1530`): `allow(dead_code)` outside test builds; 1 construction
  (`:1560`).
- `ProofSearchActivity` (`prove.rs:695`, `pub`): 0 users in `source-rust/` outside the crate (065); 2 in
  `tools/benchmark/{machine-checks,proof-preparation}/harness.rs`, which the crate's examples include (checked).
- `result_summaries::widest` (`pub(super)`): no call outside its file.
- `P06/tests/dominance.rs:1-6`: an integration target with no `#[test]`; it only compiles `graph.rs`.
- `P06/examples/proof_preparation*.rs`: compile `known.rs` and `prove.rs` a second time; comment
  `proof_preparation.rs:9` names "source settlement", a name the reader did not find. Whether they build was not checked.
- `entryless::judged_typed` (`src/tests/entryless.rs:11`): 1 external test use.
- `retained_package_review_numeric_requirements` (`P07/src/package_review.rs:47`): record 068 found no caller. Wrong
  (checked): 1 caller, `source-rust/subsystems/compiler/build/planning/package_review/numeric.rs:106`.
- One-caller functions (grepped by the records): `arrivals::admit` (`joins.rs:89`), `callable_arrivals::check`
  (`verdict.rs:1564`), `crashes::coverage_goal` (`lib.rs:2277`), `external_providers::publish` (`verdict.rs:1400`),
  `machine_batch::check` (`lib.rs:544`), `operational::judge` (`lib.rs:483`), `static_reach::validate`
  (`verdict.rs:996`), `static_reach::call_reach` (`ceilings.rs:1182`), `static_contracts::publish`
  (`verdict.rs:1403`), each public function of `rankings.rs` (065 list), `result_summaries::{infer, publish,
  checked_results}`, `build_companions::produce` (`accepted.rs:315`), `application::attach` (`accepted.rs:87`),
  `published_entry::issue` (`custody.rs:231`), `publish_package_review_numeric_requirements` (1 caller).
- "The one" claims: `call_contracts.rs:1` "one public contract source": 36 occurrences of `call_contracts::` in 10
  stage 06 files plus 1 unrelated hit (063). `prove.rs` "the one prover": 6 production construction sites (section
  3). `verdict.rs:61-64` "`fall_back_or_defect` is the one place a refusal becomes a verdict": `RouteVerdict::FallBack`
  is constructed at 6 places in `verdict.rs` (`:976`, `:987`, `:1132`, `:1361`, `:1651`, `:2010`) and matched once in
  `publication.rs:139` (checked; 068 counted the match as a seventh construction); `Rejected{..}` is also built
  directly at `:1550`, `:1573`.

## 8. Stated limits

- **Obligation routes.** "Kinds outside those five are refused as `Unsupported`" (`lib.rs:50`, while `Discharged` has
  eight variants, `lib.rs:175`); "no proof rule is yet marked as holding at every type" (`lib.rs:2125`); "obligation
  kind outside this route" (`reuse.rs:1890-1894`); kinds "refused where they are decided" (`minted.rs:1104`).
- **Verdict.** "nothing falls back since the owner's ruling of 2026-11-12" while the variant is still `FallBack`
  (`verdict.rs:19-21`); "`Incomplete` has no `Diagnostic` constructor here" (`:40-43`); only `Unsupported` says "not
  supported yet" (`:64`).
- **Calls and boundaries.** Suspension "needs a separate continuation/carry producer" (`call_contracts.rs:520-522`);
  provider obligations "still need their independent proof and publication" (`:535-537`); header shapes "currently"
  published exact (`external_providers.rs:138-139`); "a possibly suspending terminal call needs a checked suspension
  crossing" (`operational.rs:333`); static progress premises, callback crash routes, open crash dependency
  (`static_contracts.rs:128`, `:474`, `:491`).
- **Storage and loans.** Self-declared linear scans per operation (`places.rs:1415`, `:5632`); field crossings capped
  at 256 (`:7077`); "Missing immediate-parent or returned-call correspondence remains unavailable"
  (`places/loans.rs:7-9`); write-only loans (`:855`, `:1096`); restoration and child closure (`:1285`); arrival-loan
  correspondence (`:2744`); output-loan correspondence (`:2805`); mutable sequence construction and atomic residents
  (`places/log.rs:297`, `:324`, `:371`); owned byte storage (`places/byte_payloads.rs:543`, `:573`); cleanup after a
  preceding write (`places/cleanup.rs:325`); case at entry "not established by structure" (`places/contents.rs:559-562`).
- **Prover.** Three "provisional" kernel rules the kernel "trusts" (`prove/ranges.rs:30-45`); wrapping, saturating and
  trapping arithmetic at parameter width refused before the prover (`prove/widths.rs:30-32`); `CASE_BRANCH_BUDGET = 64`
  (`prove.rs:84`), `INTERVAL_CANDIDATES = 12` (`:259`); "Deeper clause expressions are outside this covered subset"
  (`:193`).
- **Inference.** "Only facts about exact actuals cross here" (`entry_inputs.rs:13-15`); unformed conjuncts left out
  (`:1217-1223`); only scalar results summarized, no summary for borrowed-formal writes (`result_summaries.rs:51`,
  `:59-60`); dispatch payload parameters get no arrival (`joins.rs:50-51`); Booleans excluded from join scope
  (`joins.rs:273-284`); recursive transfer termination argument (`entry_inputs/recursive.rs:105-107`); the authored
  head predicate is "never inferred here" (`arrivals.rs:19-24`).
- **Crashes.** A cause with no published route owes `Falsehood`, which "this prover does not do"
  (`crashes.rs:11-15`).
- **Ceilings.** Source-only signatures "not encoded, sealed or executable" (`ceilings.rs:54-55`).
- **Generics.** Factorial pairing beyond 8 clauses (`reuse.rs:1119-1123`); "the subset has no generics"
  (`reuse.rs:129-130`, stale per 065).
- **Rankings.** Closed finite integer carriers; complete reachable control graph (`rankings.rs:247`, `:140`).
- **Optimization.** Only Boolean results written back (`publication/optimization.rs:253-258`).
- **Messages.** `describe.rs:487` cites "decision 17", an owner-queue number that `AGENTS.md` calls mutable.
- **Stage 07.** Unowned calls "have no row and no slot" (`build_companions.rs:39-42`); only accepted source
  establishes protocol correspondence (`custody.rs:136-140`); review covers every declared numeric operation without
  an exact indirect-call closure (`package_review.rs:1-3`); unsupported supply kinds refuse only programs that reach
  them (`provider_supply/mod.rs:12`); "no product route yet" (`provider_supply/mod.rs:456`; `accepted.rs:286`,
  `:300`); retained input commitment and generic Product designation
  (`source_selection_publication.rs:1127`, `:1200`).
- **Pinned by tests as current refusals.** `representation_views.rs:37-107` (`TASKS.md` STATE-LOCAL-VALUE-FRONTIER);
  `qualified_terms.rs:635` (qualified scalar loans); `collection_members.rs:651` (source storage cycles);
  `loan_regions.rs:196-198`; `operational_effects.rs:283-285`, `:707-709`; `ceilings.rs:358-359`;
  `layout_queries.rs:135-137` (target layout stays a pending prerequisite); `provider_supplies.rs:251-252`;
  `floats.rs:9`; `atomic_wake.rs:333-334`; `borrowed_restoration.rs:264`, `:551-552`; `address_bounds.rs:61-64`;
  `arithmetic_policies.rs:265-270`; `array_lengths.rs:65-66`; `byte_view_elements.rs:260-262`;
  `byte_domains.rs:585-588`, `:783-785`.

## 9. Disagreements between records, and with the crate map

- **`fn verdict(` copies.** 067: 15 in `src/tests/`. 066_gap_032: 17 files under `P06/src`. Checked: 17 files, of which
  16 are in `src/tests/` and the 17th is the method `Unsupported::verdict` (`verdict.rs:1671`), not a copy.
- **`build_companions.rs` test count.** 069: 37 tests (and "36 listed"). 069_gap_033 lists 25 from line 341.
  Checked: 27 `#[test]` in the file, 24 at or after line 341. With 23 in `P07/src/`, this matches the crate map's 50.
- **TASKS item for `scalar_reference_recasts_keep_their_backing_and_presented_fields`.** Crate map: a sub-bullet of
  REQUIRED-AUTOMATIC-DISCHARGE. 067: STATE-LOCAL-VALUE-FRONTIER. Checked: `TASKS.md:220-221` puts it under
  STATE-LOCAL-VALUE-FRONTIER; the crate map is wrong.
- **Stage 06 path dependencies.** Crate map: "22 path crates" while it lists 24 names. Checked: `P06/Cargo.toml`
  `[dependencies]` has 24 `path =` entries.
- **`package_review.rs` and `Callee::Selected`.** 068 left it open; 069's reading of `call_closure`
  (`REP/lowered-program/program.rs:442`) resolves it (section 5).
- **`call_reach` with a required contract.** 066 left it open without reading `ceilings.rs`; 063 read `ceilings.rs`
  but did not connect it. Resolved in section 5.
- **`external_providers.rs` zip.** 063 open; resolved in section 5.
- **`FallBack` constructions.** 068: 7 in non-test files; actual 6 plus one match (section 7).
- **`Prover::new` users.** 065 counts `machine_batch.rs`; that call is in a test (section 3).
- **`judge_declared_rows`.** 063: "1 non-test caller"; both it and its caller are test-support (section 7). 063's
  maintainer note that "refinement of declared rows does not check suspension, blocking or termination" describes a
  test-support path only.
- **`places/cleanup.rs`.** 065 marked `cleanup_log_is_stated` as an unread suspected duplicate; 064 read it
  (`places/cleanup.rs:190`, refuses at `:220` when `AfterCleanup` rows differ). Consistent.
- **Overlapping coverage.** 066_gap_030 read `borrowed_restoration.rs:752-1021`, also read by 066_gap_031;
  066_gap_031 read `byte_domains.rs:816-1256`, also read by 066_gap_032; 069 skimmed
  `source_selection_publication.rs:2433-3629` and `build_companions.rs:341-1607`, which 069_gap_033 read in full. No
  contradictions found between the overlapping entries.
- **Crate-map statements the records confirm.** `P07/README.md` presents SCCP under stage 07, but the pass is in
  stage 06 (`publication/optimization.rs:28`, 065); the driver calls `judge_publication`, not `judge_compilation`
  directly (065, 068).

## 10. What I checked myself

1. **Claim (066):** `static_reach::call_reach` returns no reach for a call with a `required_contract`, open fail-open.
   **Code:** `static_reach.rs:311-313` returns `Ok(None)`; the caller falls back to `envelope.reach()`
   (`ceilings.rs:1182-1194`) and `envelope_for` gives the required contract's runtime declaration envelope or refuses
   (`ceilings.rs:266-279`), with an open-reach refusal at `:1195-1199`. **Partly right** (the return is as stated; not
   a skipped check). Corrected in section 5.
2. **Claim (068):** `carried` takes the first matching requirement without an ambiguity check. **Code:**
   `verdict.rs:1839-1849` uses `.find`; the record path at `:1834-1837` requires exactly one. **Matched.**
3. **Claim (068):** `retained_package_review_numeric_requirements` has no caller. **Code:** called at
   `source-rust/subsystems/compiler/build/planning/package_review/numeric.rs:106`. **Wrong.** Corrected in section 7.
4. **Claim (068):** 7 `RouteVerdict::FallBack(` constructions in non-test files, 6 in `verdict.rs`. **Code:** 6
   constructions in `verdict.rs` (`:976`, `:987`, `:1132`, `:1361`, `:1651`, `:2010`); `publication.rs:139` is a match
   arm. **Partly right.** Corrected in section 7.
5. **Claim (063):** `.zip` at `external_providers.rs:140-144` may pair lists of unequal length; open. **Code:**
   `contract_refinement_goals` (`psi/semantics/src/calls/callable_refinement.rs:331-333`) refuses unequal
   `structural_parameters` lengths and runs at `external_providers.rs:168-169`. **Partly right** (the zip is as
   described; the risk is closed). Corrected in section 5.
6. **Claim (065):** `reference_loan` returns `Ok(())` for any operation other than three formation kinds.
   **Code:** `places/loans.rs:1067-1086`. **Matched.**
7. **Claim (065):** `Prover::new` has 8 matches in 7 files outside `tests.rs`, including `machine_batch.rs`.
   **Code:** 7 matches outside `src/tests` and `prove/tests.rs`; `machine_batch.rs:94` sits in the
   `#[test] nested_batches_share_workers_and_keep_the_lowest_refusal` (`machine_batch.rs:76-100`). **Partly right.**
8. **Claim (067 / 066_gap_032):** 15 or 17 copies of `fn verdict(`. **Code:** 17 files; 16 in `src/tests/`, 1 method
   at `verdict.rs:1671`. **Partly right** (both). Corrected in sections 3 and 9.
9. **Claim (064):** `cleanup_backing` is a near copy of `Storage::backing`. **Code:** diff of
   `places/backing.rs:699-752` and `places/cleanup.rs:54-113` differs only in point location, the captured-family call
   and error text. **Matched.**
10. **Claim (063):** `entry_inputs::canonical_parts` duplicates `joins::connective`'s "flatten / unit / zero logic".
    **Code:** both apply unit/zero (`joins.rs:329-346`, `entry_inputs.rs:1088-1130`); only `canonical_parts`
    flattens and orders by codec key. **Partly right.**
11. **Claim (063):** `judge_declared_rows` has one non-test caller, `refinement.rs:81`. **Code:** both
    `judge_declared_rows` (`ceilings.rs:940`) and its caller `prove_callable_refinement` (`refinement.rs:24`) are
    `#[cfg(any(test, feature = "test-support"))]`. **Partly right** (the count is right, "non-test" is not).
12. **Claim (065):** `ProofSearchActivity` has 0 matches outside the crate. **Code:** 0 in `source-rust/` outside
    `P06`; 2 in `tools/benchmark/*/harness.rs`. **Partly right.**
13. **Claim (064):** an unresolved cycle answers `Unknown` for a window, so it does not keep the window open; open.
    **Code:** `places.rs:3644`; consumers test `== Reached::Open` (`:5943`, `:5979`); `merge` at `:498-509`.
    **Matched** (class stays open; the trace is added in section 5).
14. **Claim (065):** `reverify*` have no production caller. **Code:** test-support gate at `reuse.rs:48` and
    `lib.rs:97-100`; no caller outside `P06/src` except a doc comment. **Matched.**
15. **Claim (064):** `owned_cleanup::writes` skips an inaccessible reference leaf with no reason at the site.
    **Code:** `owned_cleanup.rs:518-527`, bare `continue`. **Matched.**
16. **Claim (crate map):** the `scalar_reference_recasts...` test is a REQUIRED-AUTOMATIC-DISCHARGE sub-bullet.
    **Code:** `TASKS.md:220-221`, under STATE-LOCAL-VALUE-FRONTIER. **Wrong.**
17. **Claim (crate map):** stage 06 has 22 production path dependencies. **Code:** 24 `path =` entries under
    `[dependencies]` in `P06/Cargo.toml`. **Wrong.**

Totals: 6 matched, 8 partly right, 3 wrong.
