# Check on the lowered form

Architecture decision, 2026-11-10: how the compiler is built. Not a language
change: the specification is untouched. Evidence was read and counted on `main`
at `042971d0288`; nothing here was built or run, and no fixture outcome is
claimed. Amended the same day after every line of `omega-rust` was read at
`457adf4a11f`; [what that read changed](#what-the-full-read-changed) lists the
amendments, and the records are in the
[compiler atlas](../../../.agents/atlas/compiler-2026-11-10/README.md).

The checker proves facts about source trees and then hands them to lowering,
which turns the trees into explicit values, places, blocks and edges. Facts are
lost, wrongly kept or rebuilt at that hand-off, and every new program shape
needs new code on both sides. This record reverses the order: lower first,
then check. Checking runs on the explicit form the compiler already emits, so
the checker and the product name the same values and nothing is carried
between them. It supersedes the design section of
[fact transport](fact_transport.md), whose evidence still stands and whose
errors are listed [below](#corrections-to-fact-transport).

## Why the order is the problem

Three parts hold the checking and lowering of a program:

| Part | Rust lines | Functions |
| --- | --- | --- |
| Psi 05, checking (`pipeline/05_typed-trees-to-checked-trees/src`) | 337,429 | 6,041 |
| Validation, called by Psi 05 (`validation/src`) | 331,088 | 7,953 |
| Psi 06, lowering (`pipeline/06_checked-trees-to-lowered-psi/src`) | 123,583 | 2,152 |

That is 55% of the Rust under `omega-rust/psi`. Psi 07 and Psi 08 are 1,812
and 668 lines.

Psi 05 already lowers. 73,031 of its lines are `execution*.rs`, which its own
header describes as "the builders that turn a checked program's control, call
and ownership facts into the plans a Terminal producer consumes". Psi 06 then
lowers again from those plans, and keeps a fact-transport engine of its own:
`unit_structural_values.rs` decides, at each lowered read, which memberships
are still current, with its own overlap check and its own walk of the write
history.

The specification states fact validity on the lowered form, not on trees.
[Terminal fact validity](../../spec/terminal-psi/fact_validity.md) speaks of a
"captured SSA value" that "subsequent writes cannot change", of an "exact
reaching value version", of merge tokens at joins and checked invariants at
cycles. [The product contract](../../spec/terminal-psi/product.md) requires
that "execution, proof questions, evidence, diagnostics, work, and lowering
refer to the same stable values, places, operations, and edges". Stage 05
re-implements those rules on a form where values, versions, blocks and edges
are not explicit, so it keys facts by expression spelling
(`display_name` labels in `checks_ranges_facts.rs`; "Contract comparison is
currently label-based" in `checks_contracts_calls.rs`) and replays evaluation
order per statement.

Three consequences were read in the code and the documents:

- **Per fact kind and place.** Four fact models coexist in the numeric prover
  (`ValueEnvironment`, `Engine` / `StructuralJudge`, `CurrentScalarValues`,
  `RangeFacts`), with about 25 key shapes built from names and debug text. At least five
  records answer "what was written between two points".
- **Per program shape.** The stage 05 README lists support by recognized
  shape: a restored call "admits a direct mutable parent with an exclusive
  child or one complete shared cohort of one to three children"; a measure
  shares range proofs for "the same bare `u8`, `u16`, `u32`, or `u64`" carrier
  "only when its body names its exact resolved parameter". Validation's README
  locates the cause: there is one general arithmetic engine, and per-shape
  adapters that bind source shapes to its atoms.
- **Into lowering.** Of 148 references from Psi 06 into validation, 8 are
  fact queries at a program point. Of 52 `CheckFacts` fields, 5 are place
  facts; the rest are plans, decisions and per-occurrence verdicts.

[Compositional lowering](../../../omega-rust/pipeline.md#compositional-lowering)
already prohibits "reinforcing required source-shape admission" and
"another required producer family for an incidental arrangement". The rule
did not hold because the representation makes the recognizer the cheapest
correct change.

## What patching costs

Counted over commits authored from 2026-11-01 to 2026-11-10 (1,140 commits,
855 in compiler lanes). A "preserve-style" title matches
`preserve|retain|carry|keep|returned|borrowed|through`; the match is crude.

- Of 1,570 ledger rows red on 2026-11-01, 948 are green on 2026-11-10, 146
  were deleted and 476 are still red. Patching works on recorded rows.
- The 291 preserve-style PRs turned 427 existing red rows green. 221 of them
  (76%) turned none; 133 only added their own new rows.
- Those 291 PRs added 176,890 lines of Rust and deleted 52,614: about 414
  lines added per existing row turned green. In Psi 05 and validation the
  ratio is 3.5 lines added per line deleted.
- Rust under `omega-rust/psi` grew from 1,346,672 to 1,437,728 lines.

The ledger understates what is broken: 9,312 of 16,788 rows carry a stamp from
2026-10-30 that is not a whole-corpus run, and 148 of 327 fact negatives differ
from their positive twin only in a numeric literal, so a checker that keeps a
stale fact still rejects them.

## The decision

1. **Lower first.** Elaborate typed trees into unsealed Terminal Psi using
   structure only: blocks, typed values, places, operations, calls, edges and
   the claims and loans their operations create. This step proves nothing and
   reads no checker fact.
2. **Check there.** Reconstruct every obligation from that module, as
   [Terminal fact validity](../../spec/terminal-psi/fact_validity.md),
   [ownership](../../spec/terminal-psi/ownership.md),
   [loans](../../spec/terminal-psi/loans.md) and
   [control flow](../../spec/terminal-psi/control_flow.md) define them, and
   discharge each with one prover over the module's values.
3. **Emit what was checked.** Publication seals the same module with its
   evidence. There is no fact hand-off, because the checker annotated the form
   that is emitted.

Psi 00 through 04 and the Omega stages keep their pipeline structure. Psi 05,
validation and Psi 06 are replaced. Because real programs (and the 7,309
`facts/` fixtures, `samples/` and `source/library/`) combine construct families
inside one machine and call graph, the shipping path switches per whole
compilation closure rather than per construct family: when every reachable
construct elaborates, the lowered-form checker's verdict (`Evidence` or a
non-`Unsupported` `Refusal`) governs; when any reachable construct returns
`RefusalReason::Unsupported`, the whole compilation falls back to Psi 05 and
06. Psi 05, validation and Psi 06 are deleted once `Unsupported` reaches zero
across the language.

Two standing rules on the parts that stay, both from the full read:

- **Identity is fixed upstream before elaboration.** Decisions keyed on a name
  or on text are not confined to the replaced stages: Psi 02, 03 and 04, the
  typed trees and build-time evaluation hold at least 451 of the 2,004 such
  places the read counted, and in the typed trees two types are the same when
  their text renderings are equal. Elaboration takes symbols and type
  identities from the typed trees, refuses where the typed trees offer only a
  spelling, and never passes a spelling on as an identity. Where a construct
  reaches elaboration as a spelling or an invalid symbol (such as `result` in
  `ensures`, or constants that build-time evaluation wrote back as generated
  names), the lane repairs the stage in Psi 02 through 04 or build-time
  evaluation that lost the identity rather than matching text in elaboration.
- **Omega's own checks are not counted as independent.** Its stages mostly
  check a result with a second hand-written derivation or with the same
  builder. This record does not change what Omega trusts, removes or guards.
  The back half trusts the checker, as the specification says; the checker
  earns that by accepting only what the proof kernel verified and rejecting
  the rest at compile time ([What each stage may trust](stage_trust.md), rule
  2). It does not lean on
  those checks to catch a defect in the new checker.

Much of the receiving side exists. `terminal-psi/semantics` (51,352 lines)
already defines "judgments, operational analyses, and structural/scalar
invariants over `TerminalModule`", shared by the verifier, the interpreter and
Omega 00. `proof/admission` (54,654 lines) checks supplied evidence and "does
not run a proof search of its own". The missing parts are a structure-only
elaboration and a prover that states its questions over Terminal values and
emits evidence the kernel checks.

### The model the checker holds

These come from the specification, not from this record:

- **Values.** Every evaluated value has one identity that later writes cannot
  change. Knowledge (ranges, equalities, relations, proved membership) is a
  row over value identities with its premises and assumption closure. It never
  becomes false; a write changes which value a place holds.
- **Bindings.** Which value each place currently holds, with a forward write
  log. A dynamic index names the value captured when the place was formed.
- **May-sets.** What a reference might denote, which machine a callable might
  select. They join by union and unknown means everything.
- **Obligations.** Claims with a current home, loans with lineage and access,
  open windows and domain debts. Each is an identity, not a fact, and every
  edge conserves them: an incoming obligation is transferred, consumed, cleaned
  up or discarded exactly once.
- **Edges and summaries.** Invocation entry is one operation for ordinary and
  terminal calls. A callee's published contract has one term per axis (reach,
  suspension, blocking, crashes, termination, carry, result relation), each
  with its own algebra.

Three rules bound it. Facts never feed type identity, operator selection or
specialization keys. Information derived from a body crosses only edges inside
"the same fingerprinted verification unit"; every other edge uses the authored
contract. Attached domains are part of the declared type and change only at a
[named cast](named_domain_casts.md).

Two more rules come from the full read, and are how this checker must differ
from the code it replaces:

- **No table is keyed by text.** Not by a name, a printed expression, a place
  path string, or a formatted handle. Text is for diagnostics.
- **Every step yields a row or a refusal with a reason.** An absent row is
  never a verdict. The read counted 930 places where a step that cannot plan
  or resolve something produces nothing and says nothing.

## Route one, before anything else is built on it

[Compositional lowering](../../../omega-rust/pipeline.md#compositional-lowering)
requires "one working, independently checked route through the new mechanism"
before variants are added. Route one is:

- **Census first.** Before any new code, count on a corpus slice how many
  operation obligations (bounds, overflow, a call's `requires`) reach the
  product with evidence the kernel checked. Reading says none do: Omega 00
  records every operation obligation in a unit as accepted, with no evidence
  attached. The census turns that reading into a number that route one must
  move. [The first count](#census-of-evidence) confirms it: of 814 obligations the
  product accepted, none carried evidence.
- **Subset.** Machines whose bodies use scalar locals, assignment, builtin
  arithmetic and comparison, direct calls with `requires` and `ensures`
  (including clauses that name `result`), `transition` arms with branch-guard
  hypotheses, explicit `crash` statements with `crashes` contracts, and
  returns. No references, records, loops or generics. Excluding `crashes` and
  `result` in the initial elaboration probe (`#2286`) left only 16 non-empty
  corpus programs (1,556 `pass/` and 153 `run/` fixtures refused on `contract
  crashes` alone), which is too small a sample to judge the gate.
- **Build.** Shared dense-identity tables (`terminal-check-tables`, `#2279`);
  structure-only elaboration of the subset to unsealed Terminal Psi
  (`structure-elaboration`, `#2286`); obligation reconstruction and discharge
  by one prover (`lowered-discharge`, `#2293`) that states goals over Terminal
  values and emits certificates verified by `proof/admission` rather than
  threading `validation`'s source-shaped arithmetic engine. Before the gate
  closes, the prover must also prove relations and affine bounds between two
  values (`i < len`, `count <= cap`), not bounds against constants alone, and
  report `Unproved` obligations by cause so later lanes never add a second
  route. In the tables, `NormalGuarantee` stays one obligation per `(return
  edge, clause)`, and a clause on `MachineContract::ensures` is sealed iff
  every `Destination::Return` edge has a kernel-accepted proof for it
  (vacuously true when there are zero normal return edges). Discharged
  certificates travel in memory beside `TerminalModule` within one compile so
  Omega 00 can fill `AcceptedObligationFact::proof_bundle_fingerprint` without
  changing what the back half trusts, removes or guards; an artifact re-read
  from bytes (including a compiler-storage cache read) has no in-memory
  evidence, which is explicit and neither an error nor filled from an earlier
  build (see [What each stage may trust](stage_trust.md)). An obligation the
  prover cannot discharge is a compile error for the program, never a slower
  guarded form.
- **Oracle.** Every corpus program inside the subset is checked by both
  routes. A disagreement is a defect in one of them and is classified, not
  averaged. The must-reject programs listed below guard the cases where the
  current checker is itself wrong.
- **Measures.** Lines of code for the route against the code it would retire;
  verdict agreement; check time per program; lines added per row when one more
  construct is added. Four more, each with a target the old code misses:
  obligations discharged with kernel-checked evidence (all of the subset's);
  decisions keyed on text (none); steps that drop without a refusal (none);
  routes that can prove one kind of obligation (one; a call's `requires` has
  about twenty today, tried in order).
- **Stop rule.** If the route is not clearly smaller than what it replaces,
  cannot reach agreement on the widened subset, or cannot prove two-value
  relations through its single certificate route, the replacement stops and
  this record is withdrawn in favor of migrating in place.

After route one passes its gate, `structure-elaboration` is split into per-family
modules and the remaining construct families run in two dependency-ordered
groups rather than ten simultaneous sessions:

1. **Places foundation first:** records, fields, cases, constants and
   expressions, so `ExactPlaceId` projections, structural roots and the forward
   write log (`places`, `roots`, `writes`, all empty in the scalar spine) are
   built once. [Places foundation lane](places_foundation_lane.md) is the
   staged plan: scope, tables, corpus slice, measures and worker brief.
2. **First parallel group (five lanes):** references, loans, windows, ownership
   and cleanup; calls and callee summaries; loops, recursion and termination;
   integers, floats, casts and domains; proofs, evidence and quotients.
3. **Second parallel group (four composite lanes, plus smaller corpus
   families):** slices, collections, indices and text; generics and traits;
   Build-selected targets and providers; multi-state machines, suspension,
   atomics and assembly; alongside `layouts`, `modules`, `host`, `filesystem`
   and `capabilities`.

Each lane proves agreement between both checkers across all corpus rows whose
constructs lie within the union of landed families, and the whole-compilation
router switches those programs automatically while falling back to Psi 05 and
06 on any `RefusalReason::Unsupported`. Psi 05, validation and Psi 06 are
deleted together once `Unsupported` reaches zero.

### Census of evidence

No obligation in this slice was backed by evidence the proof kernel checked,
at either end of the pipeline. The product accepted 814 operation obligations
in 99 compiled programs, and every accepted fact had a zero proof-bundle
fingerprint. In Psi 05 no bounded-type obligation reached a certificate. Route
one starts from zero.

An obligation counts as evidence-backed only when `proof_admission` decided it
again from a certificate or derivation. Setting `OMEGA_PROOF_MEASUREMENTS`
prints the counts. They are taken over the 591 fixtures in
`tests/omega/pass/{arithmetic,calls,contracts,ranges}`, compiled but not run,
on `omega-linux` at `e29e947fbed`. 227 fixtures reached Omega 00; 364 stop
earlier because they select no program entry, and 2 were killed.

Operation obligations Omega 00 accepted, by the Terminal operation that owns
them. None has evidence; none has a reconstructed goal.

| Kind | Accepted |
| --- | ---: |
| `PrimitiveScalarRead` | 294 |
| `ExactIntegerSubtract` | 132 |
| `ExactIntegerAdd` | 84 |
| `ByteSequenceSubslice` | 58 |
| `ExactIntegerMultiply` | 42 |
| integer divide and remainder, all policies | 94 |
| `WriteOnlyPrimitiveStore` (range) | 18 |
| `IntegerExactCast` | 18 |
| exact shifts | 20 |
| `ByteSequenceWrite` | 12 |
| other (8 kinds) | 42 |
| **Total** | **814** |

Proof-plan obligations in Psi 05, by route:

| Kind | Route | Count |
| --- | --- | ---: |
| bounded call argument | no certificate attempted | 10,349 |
| bounded state return | no certificate attempted | 6,589 |
| bounded assignment | no certificate attempted | 4,275 |
| bounded initializer | no certificate attempted | 2,483 |
| guarded transition | not decided by the checker | 218,765 |
| bounded value | not decided by the checker | 58,921 |

What the numbers do not say:

- **Why no certificate was attempted.** The certificate builder needs a
  `ProofConstraint::IntegerRange`. This slice's types use arithmetic domains
  such as `u64 in Trapping`, which do not produce one. Where those obligations
  are actually decided was not traced.
- **Which obligations a checker decided.** Psi 06 records the obligations a
  checker decided (`source_checked_obligations`), but nothing reads that list.
  It never reaches the product, so Omega 00 accepts decided and undecided
  obligations alike, and the census cannot separate them without carrying
  the list forward.
- **Call `requires`.** These obligations travel on the call and never become
  accepted facts. The count covers every machine in each module, the library
  included, so it measures the module, not the program.
- **Library code.** Psi 05 counts include every proof plan a compile checks,
  library packages among them.
- **Check removal.** No Omega 01 rewrite consumed an accepted fact, because
  none ran: these compiles select no optimizations, and no selection means no
  pass step. Builds that do select optimizations were not measured. Omega 03
  selects unguarded instructions for these facts, but the facts sit in about
  25 instruction, call and edge shapes with no shared accessor, so those sites
  were not counted.

## What must be closed regardless

These accept invalid programs, or can, on `main` today. They were read in the
code and are not yet confirmed by a run:

- The prover's check that a call guarantee still describes a copied argument
  looks only at the callee's own writes, not at writes by the call's other
  operands (`checks_contracts_prover.rs` `capture_preserved`). Three
  `fail/calls/copied_*_ensures_cannot_restore_*` programs exercise it.
- A call the flow builder cannot match records no write
  (`flow_expression.rs` `invoke`).
- Index segments compare by expression structure, so a fact on `a[i]` can
  outlive a change to `i` (`validation/src/places.rs`).
- Omega 00 turns every operation obligation reference in a unit into an
  accepted fact, with no evidence attached
  (`00_terminal-psi-to-abstract-operations/src/optimization.rs`
  `source_checked_obligation_facts`); the interpreter still checks. A wrongly
  kept fact therefore makes native code and the interpreter disagree. An
  earlier wording said the back end read a list of source-checked obligations
  from lowering; no reader of that list was found.

The full read added seven places that accept on missing or mismatched input.
Each was traced on 2026-11-10 at `c118c914794`, and the probes were run on the
Mac mini. One accepts an invalid program; the other six are closed or not
reached:

- **Accepts an invalid program.** The default-domain driver skips its
  end-of-state window check when the state contains any crash statement, even
  one that cannot run (`domains_and_predicates.rs:13606`, `:13689`). A write
  that breaks a data `requires`, then a complete dispatch whose arms all
  return, then a dead `crash Trap;` is accepted. Without the crash, the same
  program is rejected. Witness: `pending/dependent/data_requires_return_beside_dead_crash`,
  accepted by `omega --check`. A crash beside a normal exit on another path cannot be
  written: the parser makes `crash` an unguarded statement that ends the
  dispatch.
- A linear carry fact crosses a call when its place label text equals the
  argument's spelling (`flow_calls.rs:419`). This adds nothing beyond
  `places_match`, which already compares labels made from bare symbol names
  (`facts/src/lib.rs:1827`). Two distinct locals or parameters cannot share a
  name: `value_custody_core.rs:5750` and `declarations_core.rs:2373` reject
  that. Label collisions remain only through index or expression spelling,
  which is the `places.rs` item above.
- An `ensures` subject binds to a callee parameter by name alone
  (`flow_calls.rs:1066`). This is closed: the parameters and the contracts come
  from one signature, and duplicate parameter names are rejected
  (`declarations_core.rs:2373`).
- With no checked body to consult, the fence on a call's result domains
  answers yes (`flow_calls.rs:546`). For a direct call, the callee's own exit
  refuses a forged `ensures result in D` (`checks_contracts_exits.rs:9902`).
  The bodyless-realization case was not run.
- A recognizer for two-arm guarded values compares the first arm's condition
  with itself (`contract_entailment.rs:44311`). Read only: this fails
  closed. The recognizer is a fallback, tried only when the primary case walk
  returns nothing, and the primary walk's refusals stay in the same batch
  (`:90466-90474`). Its two arms must be the state's only statements, both
  value returns. Each arm is judged with no hypothesis from its guard, and the
  claim is proven only when every arm proves it.
- A storage validity check answers "valid" when no storage graph exists
  (`contract_entailment.rs:73358`). Read only: this branch is never reached. No storage
  version is produced without a graph, and all seven callers are guarded.
- An arena returns a shared dummy row for an invalid or stale handle
  (`arena.rs:1023`). An invalid expression handle therefore reads as the
  literal `0`, and the operator `requires` check compares that text
  (`checks_operators.rs:2319`). An operand `0` was still refused, by the
  call `requires` check (`checks_contracts_calls.rs:3508`). Other tables'
  dummy rows were not audited.

The carry-label place, the arena place and the `places.rs` item share one
mechanism: they decide that two values are the same by comparing their
spelling. Each is closed today only by the check that stands behind it.

A review of the automated accept-on-unknown trace named nine more candidates.
Each was run with `omega --check` at `10d773ae305` on 2026-11-10. None
accepts an invalid program. Two of them really do miss on unknown input and
are covered only by a check in another file:

- **Extent search budget.** The inline-extent search gives up after 4096 steps
  and reports "no runtime extent" (`value_custody_core.rs:9551`). A data type
  that holds a generic doubled ten or more levels deep, beside
  `[u8; Count]` with a runtime `Count`, loses the "runtime value ... for an
  inline fixed-array extent" diagnostic. At depth 9 the diagnostic is still
  reported. The program is still rejected by the cleanup-disposition check
  (`flow.rs:10785`). With a const extent, the same deep type is accepted, so
  that rejection depends on the runtime extent, not on the depth.
- **Transition handback.** The scalar-loan handback check skips
  `CallSite::TransitionNamed` (`contract_entailment.rs:21217`). Lending
  `&mut target.count` to a nested or top-level machine through `transition`
  still reaches the same check and is rejected (`:21352`). A callee that
  `ensures slot == 0` is accepted. No transition shape reaching `:21217`
  was found.

The other seven are closed:

- Field signedness from the first machine (`proof/src/checker.rs:5446`).
  This cannot be written, because a `self` receiver in a machine body is
  rejected by the parser (`expressions.rs:1728`).
- Assembly that raises a rank before recursion. Rejected by the ranking check
  (`checks_termination.rs:8113`). The same assembly writing another local is
  accepted.
- A live crash beside a return. A partial dispatch is rejected as
  fall-through (`equality_and_exhaustiveness.rs:984`), and an arm cannot be
  `crash` (parse error, "expected `->`").
- A reference local lending a restricted field, including one chosen in a
  `match`. Rejected at `contract_entailment.rs:21352`.
- A guard on one local reused for a same-spelled call initializer of another.
  Rejected by the call `requires` check (`checks_contracts_calls.rs:3508`).
  Guarding the second local itself is accepted.
- The binding-observation retry. A read through a view after a write to its
  owner is rejected by the borrow-conflict check (`checks_borrows.rs:4709`).
- Atoms keyed on `Debug` text (`contract_entailment.rs:58147`). Settled by
  reading. Each `Debug` impl omits only the fields that semantic equality
  ignores, so two terms that are not equal never print the same text.

Must-reject programs the corpus lacks, none under `facts/`: knowledge that
survives a pause; a write through a partly overlapping window; a linear value
consumed on a loop back-edge; a copied scalar whose source a later operand
overwrites; a rank premise across a pause.

## Open questions

- **Generic bodies.** Terminal Psi is "concrete, and post-instantiation", and
  the specification requires a generic body to be valid for every admitted
  selection. The generics lane will first test elaborating uninstantiated
  generic bodies into the same unsealed `CheckTables` form with placeholder
  (rigid) type, constant and machine parameters checked once against their
  declared requirements and never encoded to `PSITERM\0` bytes, while concrete
  specializations emit concrete modules; keeping a tree-level check is the
  fallback only if that fails.
- **Evidence coverage.** Validation's automation "does not emit kernel
  evidence for every accepted obligation". How much of it can emit evidence
  the admission kernel accepts is not measured across the full language yet;
  for the scalar spine, `#2293` emits certificates directly into
  `proof/admission` rather than reusing `validation`. Integer range proofs in
  the current pipeline have a certificate path the kernel decides again and an
  older interval path it never sees (`proof/src/checker.rs`). Correction to an
  earlier reader-reported note here: in `proof/admission/src/bounded_denotation.rs`
  (`:8476-8556`, `:8640-8652`), the kernel front end checks the rule instance
  semantically (`integer_bound_rules`) before `rule_instance` mints a
  placeholder rule-axiom term in the core denotation; it is not an unchecked
  acceptance. Missing evidence never keeps a runtime guard in native code: the
  checker rejects what it cannot prove, and the back half trusts what the
  checker accepted.
- **Identity of a type on the lowered form.** Terminal Psi compares nominal
  identities by text within `TerminalIdentityIndex` (`terminal-psi/src/identity.rs:22`),
  and names record fields in `StructuralPathSegment::Field` by that index.
  Decided in `#2279` and `#2286`: `terminal-check-tables` mints dense
  `IdTable` identities and resolves fields once at elaboration to
  `StructuralFieldId`, and `structure-elaboration` classifies builtin scalars
  through `SymbolTable::builtin_type_atom` on the resolved root symbol rather
  than by spelling.
- **Diagnostics.** Errors must name source constructs. `LoweredSourceCustody`
  covers call occurrences only and depends on `checked-trees`, so `#2286`
  carries per-machine `MachineSources` over dense identities with explicit
  `Unpositioned` reasons; whether that suffices for every diagnostic across the
  remaining families is checked as each lane lands.
- **Size.** That the replacement is far smaller than 792,000 lines is the
  premise of this record and is unmeasured until route one's gate.
- **Reuse of checked results.** [`generics.md`](../../spec/language/generics.md#closed-applications-and-canonical-identity)
  requires checked derivations to be carried through specialization without
  re-searching under a consumer's smaller discovery budget;
  [`validation_subjects.md`](../../spec/build/validation_subjects.md#what-this-rule-does-not-do)
  says "No result is reused". Recorded for the owner as
  `specialization-certificate-reuse` in
  [`OWNER_QUESTIONS.md`](../../../OWNER_QUESTIONS.md), together with the
  reading to confirm (carry certificates through specialization and re-verify
  them in `proof/admission` in each consumer without re-searching). Read in
  context the two clauses agree: the validation chapter's own account of a
  check is "a lemma with valid evidence", and "a generic body under its
  requirements", which also points to checking a generic body once. A test of
  re-verifying a saved proof, including stale and misplaced ones, is planned
  in `lowered-discharge` before anything is built on it.

## Corrections to fact transport

- "A new version of any place a fact reads ends it" is wrong. A fact about a
  value stays true; a write moves the place. Psi 05's `CurrentScalarValues`
  and the restored-scalar rule both require this.
- The row "Build / generic selection: same arrival join" is unsound. Every
  implementation is checked once against its requirement's contract.
- "202 `psi:` PRs" cannot be reproduced: `git log --since` drops 320 commits
  to committer-date skew.
- Step 6, "stage 06 reads environment rows", covers 8 of 148 references.
- Obligations are not a second class of fact.
- The one shared place relation that work added (`place_relation` in
  `validation/src/places.rs`) has no caller outside its own file; 47 mentions
  go to `canonical_place_segments_may_overlap` instead. The `check-rules`
  advice that tells authors to call it describes a route nothing uses.

## What the full read changed

On 2026-11-10 all 2,211,695 lines of Rust under `omega-rust` were read once
and recorded, then condensed per area. The digest writers opened 401 of the
readers' claims in the code: 329 matched, 38 were partly right, 33 were wrong.
The findings below are counts of places cited, not of defects.

| Finding | Count | Where it is heaviest |
| --- | --- | --- |
| Decisions keyed on a name or on text | 2,004 | validation 508, Psi 05 227, Psi 02 155, typed trees 127, Psi 06 91 |
| Steps that drop silently | 930 | validation 143, Psi 05 129, typed trees 61, Psi 06 56 |

What that does to this record:

- **It supports the diagnosis and widens it.** The stages to be replaced hold
  826 of the 2,004 text-keyed places, and both checkers store facts under
  strings: validation under place-path strings, Psi 05's range facts under the
  printed expression. The same habit runs through the stages that stay, which
  is why identity is now a condition on the decision.
- **The shape axis is larger than the READMEs showed.** Validation's first
  half alone computes integer type limits at 11 places and has six interval
  arithmetic stacks and seven provers of "the measure strictly decreases".
  Psi 05 has about 18 walkers of Boolean expressions.
- **Refusals are often silent**, so a red row is frequently a missing plan row
  and not a reported error. The row-or-refusal rule answers that.
- **The kernel covers less than this record assumed**, so the census is now
  the first step of route one.
- **Nothing in the read measures the replacement.** It shows what the current
  code is, not that the new checker will be smaller. The stop rule stands.

## Method

Stage sizes: line and `fn` counts over `*.rs` at `042971d0288`. Ledger fates: a
set comparison of `tests/omega/corpus_outcomes.txt` at `b6f5ab1b00d` and
`042971d0288`, green meaning `checked` for pass and run rows and
`rejected expected` for fail rows. Per-PR flips: the ledger diff of each
commit, counting a row only when that status changes; a flip is credited to
the commit that recorded it. Lines: `git log --numstat` over the 291
preserve-style commits, which avoids the file moves of the 2026-11-08
consolidation. Reading: the specification chapters cited above, both stage
READMEs, `TASKS.md`, and eight read-only sweeps of the code, the history and
the corpus.

The full read: 113 readers each read a unit of at most 22,000 lines at
`457adf4a11f` and wrote a record with a file and line for each claim; 31 more
covered ranges the first pass left unread; 15 writers condensed the records
per area and tested claims in the code. The counts in
[what the full read changed](#what-the-full-read-changed) are sums of the
digests' per-crate counts. The seven places listed under "what must be closed
regardless" were opened by the author of this record; statements marked
reader-reported were not. The
[atlas README](../../../.agents/atlas/compiler-2026-11-10/README.md) gives the
method in full, what kinds of reader claims were wrong, and the known gaps.
