# Record: unit_066 (area 04-source-stages-06-07)

P06 = `source-rust/subsystems/compiler/source-to-psi/pipeline/06_lowered-program-to-checked-program/src`.
This unit is `static_reach.rs` (361 lines), `tests.rs` (7,575 lines) and 15 files under `tests/`
(address_bounds ... calling_policies). I ran out of room partway through `tests.rs`: see Coverage. The
15 files under `tests/` were NOT read, so they have no entry beyond their names and line counts.

## 1. `P06/static_reach.rs` (1-361), read in full

**Purpose.** Stage 06 check for generic callables whose callbacks ("static reach" parameters) are
substituted at call sites. It checks that each callback substitution answers its full refinement
question, that the recorded reach summaries equal what the lowered calls give, and that each generic
callable declaration commits to that reach. It also supplies the reach of one call to the ceiling check.
Header comment: "A template declaration records that function of its callbacks; it never supplies
executable code" (`static_reach.rs:3-4`).

**Shape.**
- `fault(machine, detail)` (`:10`): builds a `Refusal` with `RefusalReason::ForeignIdentity`, `Construct::Machine`.
  Every refusal in this file uses it, so every failure here is a route defect, not an unproved obligation.
- `validate(&Elaboration) -> Checked<()>` (`:19`), `pub(crate)`. One caller: `verdict.rs:996`
  (grepped `static_reach::(validate|call_reach)` over `source-rust`: 1 site), whose error goes through `fall_back_or_defect`.
- `ReachContribution { services: Vec<ServiceId>, open: bool }` (`:292`). One user, `ceilings.rs:1182` (grepped).
- `call_reach(elaboration, caller, site, inputs) -> Checked<Option<ReachContribution>>` (`:299`). One caller, `ceilings.rs:1182` (grepped).

**Inputs and outputs.** Reads `Elaboration` tables: `generic_callable_declarations`, `generic_binders`,
`requirements`, `undeclared`, `static_reach_parameters`, `static_reach_applications`,
`static_reach_summaries`, `static_conformances`, `requirement_ceilings`, `module.machines`,
`module.identities`, `module.interfaces.services`, `module.execution.callables`. Calls
`crate::checking::static_contracts::{requirement_declarations, check_envelope}` and
`terminal_semantics::static_reach::{derive, applied_reach, template_commitment}`. Mutates nothing;
returns `Ok(())` or one refusal.

**How it decides (`validate`), keyed on identity (machine ids, requirement ids, conformance index), not spelling,
except where noted.**
- `:21-30` generic declarations must be strictly ordered by `machine` (sorted-slice invariant, position).
- `:33-55` when `generic_binders` is non-empty, every machine with no `declaration` must be in `requirements`,
  `undeclared`, or `generic_callable_declarations` (binary searches by machine id); otherwise "generic body lost its complete source contract".
- `:56-62` early `Ok(())` when all four static-reach tables are empty.
- `:70-77` each parameter's `requirement` must have a complete `instance` header.
- `:78-120` for each application, walks arguments with an explicit stack (`pending`): the callback's
  originating declaration, its applied `instance`, and `static_conformances[argument.conformance]` must exist; then
  `required` (with `proof.id` overwritten by the instance's) must equal `instance.header`, and `envelope`,
  `declaration`, `requirement` and `StaticImplementation::Machine(selected)` must match. Comparison is by value
  equality of header structs and ids.
- `:121-129` every requirement must have a row in `requirement_ceilings`.
- `:130-155` recomputes reach with `terminal_semantics::static_reach::derive` and requires it to equal `static_reach_summaries`.
  A `Ceiling::Inferred` requirement yields `None` bounds; `Authored(reach)` yields the slice (`:139-142`).
- `:156-288` per generic declaration: exact body exists (`:158-162`); the body has blocks, no `declaration`,
  `generic_binders` non-empty, not an abstract requirement, no `specialization`, and no machine envelope (`:164-175`);
  published suspend/block flags equal the machine's (`:176-183`); the declared `reach` equals the derived summary and the
  projection requirement list equals `summary.parameters` (`:184-200`); each projection names a registered
  parameter and the origin's `member` (`:201-224`); each projection `member` has a non-empty portable identity text (`:225-237`);
  `check_envelope` passes (`:239`); the envelope's service invocations equal `published_synchronous_entries` (`:240-253`);
  suspend/block equal published flags (`:254-261`); finally the recomputed `template_commitment` equals `declaration.declaration.contract` (`:262-287`).
  The commitment looks up service identity text by service id (`:267-279`).

**`call_reach` decisions.** Returns `Ok(None)` (no contribution) when the callee is not
`Callee::Declared(TargetMachineCallable::Machine(_))` (`:305-310`), when `required_contract` is `Some` (`:311-313`), or when
there is neither an application row for `(caller, site)` nor a summary for the callee (`:325-333`). With a summary but no
application it uses an empty-argument application (`:314-319, 326-332`). Application `callee` must equal the called machine (`:335-340`).
For each callback requirement in the applied reach: `Inferred` sets `open = true`; `Authored(reach)` adds services (`:348-357`). Output is sorted and deduped.

**Cases by shape.** Only machine callees reach the summary path (`:305-310`); other callee kinds return `None`.

**Same job elsewhere.** `check_envelope` and `requirement_declarations` live in `P06/static_contracts.rs` (not in this unit; cited, not read).
Suspected: the ordering/`windows(2)` sortedness check duplicates other table-order checks in P06 (not grepped).

**Unused code.** `validate`: 1 call site; `call_reach`: 1 call site; `ReachContribution`: 1 consumer (grepped above).

**Stated limits.** None in the file.

**Fail-open risks.**
- `call_reach` `:311-313` returns `Ok(None)` (no reach added) when `required_contract.is_some()`. The caller's reach then lacks the
  callee's reach from this path. No comment at the site gives a reason. Status: open (I did not read `ceilings.rs:1182` to see whether another path covers a call with a required contract).
- `call_reach` `:333` returns `Ok(None)` when a declared machine callee has no application and no summary: the call adds nothing here. That is the right result for a non-generic callee; a generic callee that lost its summary is caught by `validate` (`:150-155`) rather than here. Status: inferred, not traced.
- `:56-62` early return skips every check when all four tables are empty. A program whose generic body has no entries gets no check here. Status: open (the earlier `:33-55` check still runs when `generic_binders` is non-empty).
- Otherwise none seen: every mismatch returns a refusal.

## 2. `P06/tests.rs` lines 1-4950, read in full

**Purpose.** The crate's main unit-test file. It builds scalar and record programs as closed bodies, lowers them with the stage-05
lowering (`crate::lowering::lower_bodies`), runs `discharge` / `judge`, and checks verdicts, knowledge rows, write logs and
that the proof kernel re-decides each certificate (`:1-4` header comment). It declares 75 test modules (`:28-102`) and holds
the shared fixture builders that those modules use through `super::`.

**Shape (helpers; `file:line` in `tests.rs`).**
- Expression builders: `expression` `:124`, `literal` `:133`, `unsigned` `:137`, `slot` `:141`, `strict` `:145`, `call` `:158`,
  `returns` `:172`, `compare` `:731`, `guarded` `:743`, `of_type` `:1417`, `place` `:1426`, `read` `:1431`, `point_of` `:1441`, `bind` `:1450`,
  `store` `:2491`, `declare` `:2516`, `and` `:2526`, `case_of` `:3401`, `payload` `:3406`, `is_case` `:3417`, `matching` `:3427`, `literal_of` `:4131`.
- Program wrappers: `Machine` `:205` (scalar machine) with `machine` `:216`, `lower` `:229`, `run` `:263`, `rejected` `:270`;
  `Shaped` `:1456` (record-typed machine) with `elaborate_over` `:1947`, `elaborate_places` `:1475`, `run_places` `:2043`, `run_sums` `:3446`,
  `run_constrained` `:4113`, `closed` `:4141`.
- Type tables: `records()` (Point, Line) `:1384`, `affine_records()` `:1479`, `sums()` (Point, Shape, Holder) `:3348`,
  `constrained()` (Bounded, Span, Wrap, ByteRead, Gated, Paired with data `requires` clauses) `:4028`.
- `declare_body_models` `:1985-2041`: gives each raw test machine a fake `CheckedMachineDeclaration` and an empty
  `TerminalMachineEnvelope` (no services, no suspension, no blocking, `NotGuaranteed` termination). The identity strings are
  `format!("lowered-body-model::{}")` and `format!("{value_type:?}")` (`:2004-2010`); these are test-only spellings.
- Kernel check: `assert_kernel_accepted` `:329-380` re-decides the certificate with `proof_admission::verify_obligation`
  and requires a tampered conclusion (`Proposition::Falsehood`) to be refused; `assert_kernel_checked` `:313` also requires
  `MathematicalCoreDecision::Judged`. `assert_unproved` `:382`, `assert_opens_a_window` `:397`, `certificate` `:1062`, `uses` `:2531`,
  `verdicts` `:282`, `store_by_hand` `:2547` (inserts an operation and a write row by hand to model a store the source cannot write),
  `read_equals` `:2075`, `assert_read_equation` `:2100`, `read_premises` `:4609`, `dispatch_premises` `:4691`, `bound_payloads` `:3469`.

**Inputs and outputs.** Read `crate::lowering::*` (stage-05 lowering re-exports), `crate::checking::*` (`discharge`, `judge`, `places::lineage`,
`places::overlap`, `graph::Graph`, `has_rules_for`, `published`), `terminal_semantics::plain_affine::{plan, verify}`, `proof_admission`.
They only assert; no production state is written.

**Tests in this range, by topic (all `#[test]`; line of the `fn`).**
- Arithmetic and call proof: exact add/sub/mul/div bounds `:425-558`, call requirement `:574-594`, normal guarantee `:605-621`,
  knowledge justification rows `:624`, arm operation dominance via `Graph::dominates` `:650`, true/false branch bounds `:752, 798`,
  exactly one verdict per obligation `:816`, relational bounds (`i < len`, transitivity) `:970-1037`, published clause needs every return `:1075`, vacuous published clause `:1112`, disequality guard `:1127`.
- Route verdicts: accept only when all discharged `:1155`; unproved diagnostic text `:1172-1237` (asserts exact message prefixes
  such as "cannot prove requires clause 1 of the called machine. (", and "decision 17 -- exact arithmetic is a proof obligation"; so these tests key on message text);
  `Unsupported` falls back, `Violation` rejects, `ForeignIdentity` is an error `:1240`;
  `has_rules_for` covers integer goals only, not bool equality or Wrapping/Saturating terms `:1293`; no-rule goals give `Incomplete` `:1333`, mixed gives `Rejected` `:1351`.
- Affine (move-once) records: `:1489-1945`, including tampered cleanup lists, order reversal, multiplicity drift and an edge id of `u64::MAX`
  (`:1851`, asserts planning stays bounded), expecting `RefusalReason::ForeignIdentity` for each forgery.
- Places and reads: constructed operand `:2113`, formal leaf `:2150`, copy and call-result leaf `:2208`, record actual `:2274`, arrival difference `:2324`,
  overlap of siblings/ancestors/cases/fixed indices `:2414`, store and sibling `:2617`, guarded-arm store `:2666`, store before branch `:2724`, whole assignment `:2756`.
- Unestablished use: gated local read `:2832, 2871, 2898`, scalar and record zero-initialised locals `:2950, 2990`, store on one way to a join `:3055`, exactly one verdict per use `:3193`.
- Sums: payload binding `:3490`, case overlap `:3581`, payload read `:3632`, membership test and retirement by a whole write `:3711`, dispatch edges `:3821`, missing declared case `:3850`, join drops arm facts `:3881`, sum local zero is first case `:3924`.
- Data `requires` (DataDomain): stale local `:4156`, sibling literal `:4188`, field store `:4236`, call-returned field `:4321`, nested store `:4363`, case-testing clause `:4420`, zero-satisfies-clause local `:4484`, gated type `:4515`, one verdict per obligation `:4574`, clause lent to reads `:4625`, dispatched payload `:4745`, built payload lends none `:4783`, written place lends none `:4822`, type built around arrived value `:4876`, call result carries type clause `:4909`.

**How tests decide.** Keyed on identities: `ObligationId`, `ValueId`, `OperationId`, `ExactPlaceId`, `ProgramPoint`, `Justification` variants, and `RefusalReason`.
Exceptions that key on text: message-prefix asserts (`:1181-1184, 1202, 1233, 1341-1345`), the detail string
`"owned storage is used after its value was transferred or consumed"` (`:1643-1644`), `"cannot prove selected case for payload access: a path to this access does not establish the case"` (`:3844`),
`"a dispatch lists no edge for a declared case"` (`:3876`), and `detail.contains("exceeds its output")` / `contains("borrowed storage")` (`:1792, 1816, 1942`).

**Cases by shape.** Test programs are limited to what `Statement` allows: comment `:2323-2333` "The source language has no statement inside the `!b` block, so a store there is added by hand";
`:1112-1114` "The subset cannot write a machine with no normal return, so the tables state one directly". `has_rules_for` test pins that only integer goals have rules (`:1293-1330`).

**Same job elsewhere.** The helper vocabulary (`Shaped`, `closed`, `elaborate_over`, `bounded`, `verdicts_of`, `uses`) is shared with the `tests/*.rs` modules via `super::` (suspected; `tests/` files not read).
`declare_body_models` (`:1985`) models declarations for raw bodies; suspected duplicate of source-driven declaration custody elsewhere in `P06/tests/support/source_handoff.rs` (not read).

**Unused code.** Not applicable (test file). `pub(crate) mod rankings;` (`:81`) is the only non-private module; other test files may use it (not grepped).

**Stated limits.** Quoted: "Raw-body units model declarations explicitly; they do not test source custody" (`:1982`). "(Until the owner settled the data half of `local-without-initializer`, the second was `Unestablished`.)" (`:3009-3011, 3929-3930`) shows an earlier ruling changed these expectations.
`:4322-4325` references issue "#3000" for result-leaf naming.

**Fail-open risks.** These are tests; they do not accept anything. Things tests pin as accepted that a reviewer could check: the vacuous published clause with no normal return holds (`:1112-1124`, stated: "holds vacuously");
an `Ok(Discharged::Window(_))` verdict for a gated declaration `:4529`, `:4551`, `:4602` (a window means a clause not proved at the store; the test says the read of the zeroed storage is then `Unestablished`, `:4537`).

## 3. `P06/tests.rs` lines 4951-7575: NOT READ (2,625 lines)

## 4. Files under `P06/tests/`: NOT READ

`address_bounds.rs` (319), `applications.rs` (329), `arithmetic_policies.rs` (340), `array_lengths.rs` (871), `arrays.rs` (415),
`atomic_wake.rs` (350), `borrowed_restoration.rs` (1021), `boundary_calls.rs` (4385), `branch_warnings.rs` (174), `build_purpose.rs` (356),
`byte_domains.rs` (1256), `byte_view_elements.rs` (2074), `call_claims.rs` (147), `callable_contracts.rs` (760), `calling_policies.rs` (1077).
The names come from `tests.rs:28-42`. I know nothing else about their contents.

## Coverage

- Read in full: `static_reach.rs` 1-361 (361 lines); `tests.rs` 1-4950 (4,950 lines). Total 5,311 lines.
- Skimmed: none.
- Not read: `tests.rs` 4951-7575 (2,625 lines); `tests/address_bounds.rs` 1-319; `tests/applications.rs` 1-329; `tests/arithmetic_policies.rs` 1-340;
  `tests/array_lengths.rs` 1-871; `tests/arrays.rs` 1-415; `tests/atomic_wake.rs` 1-350; `tests/borrowed_restoration.rs` 1-1021;
  `tests/boundary_calls.rs` 1-4385; `tests/branch_warnings.rs` 1-174; `tests/build_purpose.rs` 1-356; `tests/byte_domains.rs` 1-1256;
  `tests/byte_view_elements.rs` 1-2074; `tests/call_claims.rs` 1-147; `tests/callable_contracts.rs` 1-760; `tests/calling_policies.rs` 1-1077.
  Total not read: 17,034 lines (2,625 + 14,409).

## Five things a maintainer most needs to know about this unit

1. `static_reach.rs` is a pure checker over `Elaboration`: all failures are `ForeignIdentity` refusals (`:10-17`), so a bad generic reach record is a route defect (`verdict.rs:996` maps it through `fall_back_or_defect`), never an "unproved" user error.
2. `validate` recomputes reach with `terminal_semantics::static_reach::derive` and requires equality with the stored summaries (`static_reach.rs:130-155`), and recomputes the template commitment per declaration (`:262-287`); the stage-05 data is trusted for nothing but ordering and ids.
3. `call_reach` returns no contribution when the call carries a `required_contract` (`static_reach.rs:311-313`) with no comment saying why; its single caller is `ceilings.rs:1182`.
4. `tests.rs` tests are built on the lowering API plus hand edits to the lowered module (`store_by_hand` `:2547`, cleanup-list reversal `:1703`, multiplicity drift `:1720`); forgery tests expect `ForeignIdentity`, and unestablished storage use expects `Unestablished`, so the three verdict classes (`Unproved`, `Unestablished`, `ForeignIdentity`) are pinned here.
5. Several tests pin exact diagnostic text (`tests.rs:1181-1184, 1202, 1233, 1341, 3844, 3876, 1643`), so changing message wording breaks them.

## What I am unsure about

- Whether `call_reach` returning `None` for a call with `required_contract` (`static_reach.rs:311`) is covered by another path in `ceilings.rs`; I did not read `ceilings.rs`.
- Which of the 75 modules declared at `tests.rs:28-102` share helpers from `tests.rs` beyond what I saw, and whether `pub(crate) mod rankings` (`:81`) is used outside `tests`; not grepped.
- Test counts: I did not run a count of `#[test]` in the part of `tests.rs` I read.
- The contents of the last 2,625 lines of `tests.rs` and of all 15 `tests/` files.
