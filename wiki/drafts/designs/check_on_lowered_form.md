# Check on the lowered form

Architecture decision, 2026-11-10: how the compiler is built. Not a language
change: the specification is untouched. Evidence was read and counted on `main`
at `042971d0288`; nothing here was built or run, and no fixture outcome is
claimed. Amended the same day after every line of `omega-rust` was read at
`457adf4a11f`; [what that read changed](#what-the-full-read-changed) lists the
amendments, and the records are in the
[compiler atlas](../../../.agents/atlas/compiler-2026-11-10/README.md).
Amended 2026-11-11 after an outside review: route one's stop rule could not be
evaluated, so [a four-condition gate](#the-gate) replaces it. The owner lifted
the pause on 2026-11-12: construct work proceeds alongside the four conditions.
Later on 2026-11-12 the owner ruled that Psi 05, validation and Psi 06 are
removed now rather than once every program is covered. That ruling supersedes
the fall-back described in earlier revisions of [The decision](#the-decision),
[The gate](#the-gate) and [Lane order](#lane-order); those sections state the
current rule, and passages below that still describe two checkers shipping
together record the state before it.

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
inside one machine and call graph, the lowered-form checker judges a whole
compilation closure, never one construct family of it, and its verdict is the
compilation's only verdict (owner ruling, 2026-11-12). When every reachable
construct elaborates, the result is `Evidence` or a non-`Unsupported`
`Refusal`. When any reachable construct returns `RefusalReason::Unsupported`,
the compilation is refused with an error that names the construct and says the
checker does not support it yet. It is never handed to Psi 05 and 06 for a
verdict.

Psi 05, validation and Psi 06 are deleted now, not once `Unsupported` reaches
zero. The owner's reason is the [census of evidence](#census-of-evidence):
none of the obligations the old checker accepts is backed by evidence the
proof kernel re-checked, so a program accepted by it and then built was never
a checked program. The cost was stated and accepted: a program the route does
not govern yet, and a program that needs the product hand-off from the
route's own module, stops compiling until the route covers it. No pull request
edits the old stages except to delete them.

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
specialization keys. Additional semantic facts derived from a body cross only
exact checked call edges inside the same fixed, fingerprinted verification unit;
every other edge uses the published contract, including its existing inferred
operational, ownership and footprint information. Abstract requirements remain
abstract even within the unit. Unit membership and body-summary dependencies
follow [local call summaries](../../spec/language/state_contracts.md#local-call-summaries),
not optimizer inlining or incidental source availability. Attached domains are
part of the declared type and change only at a [named cast](named_domain_casts.md).

The 2026-11-11 decisions `callee-result-body-summary` and
`nested-helper-caller-facts` also restrict input discovery: only private checked
helpers with omitted `requires` may infer additional entry conditions, and all
arrivals and callable conversions check them. Written preconditions are complete;
public and required input contracts never narrow to excuse the implementation.
The [delivery plan](local_inference_and_contracts.md) pins routine local inference
and separately verified traversal reuse, without requiring arbitrary contract
discovery or changing kernel-checked acceptance.

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
  (`typed-trees-to-terminal-psi`, `#2286`); obligation reconstruction and discharge
  by one prover (`terminal-psi-to-terminal-psi`, `#2293`) that states goals over Terminal
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
  meanings one kind of obligation has (one, wherever the value sits;
  [restated below](#one-meaning-per-obligation)). [Route one measures](route_one_measures.md)
  records all eight and the verdict against the stop rule this record first
  carried.
- **Gate.** [The gate](#the-gate) below. It replaces the stop rule, which
  could not be evaluated.

### The gate

**Superseding ruling, 2026-11-12 (later the same day).** The old checker is
removed now. The router (`products/src/terminal_publication.rs`, `ProductRoster::judge`)
takes the route's verdict as the compilation's only verdict:

- The route rejects: the compilation fails with the route's diagnostics. The
  old checker's diagnostics no longer follow them.
- The route cannot judge the program (`RefusalReason::Unsupported`, an
  `Incomplete` verdict, or a defect of the route): the compilation fails with
  an error that opens "not supported yet" and names the construct or goal. The
  program is not given to Psi 05. A missing verdict is a refusal, never an
  acceptance.
- The route accepts: Psi 05 and 06 still run until they are deleted, only
  because product selection, the provider inventory and target preparation
  read their output. They also still judge the Build program and the bound
  target's entry-contract source, which the route does not lower yet, and a
  rejection placed there stands. A rejection placed anywhere else fails with
  "no product route yet".

Deleting Psi 05 and 06 removes that last use. From then on a product exists
only through the route's own published module (`AcceptedClosure::publish`),
so that hand-off is the critical path, and until the route lowers Build
programs no compilation completes. The four conditions below no longer decide
whether the old stages may be deleted. Conditions 1 to 3 remain as statements
of what the route must get right; condition 4 is replaced by the deletion
itself. The paragraphs below that describe the router while both checkers
shipped are the record of that earlier state.

The owner lifted the pause on 2026-11-12: the construct lanes resume now, and
the four conditions below are pursued alongside them rather than before them.
Only the owner and Zac use the compiler today, so a lane that lands ahead of a
condition costs nothing a user would see.

The route was not to grow to another construct family until one bounded
experiment passed. The experiment has four conditions, and it passes only when
all four hold on `main`. Each one answers a question the route has not yet
answered about itself: whether it checks the right declarations, whether a
mistake in its own list of proof duties would be noticed, whether it can
handle generic code at all, and whether it lets any old code be deleted.

This replaces the stop rule this record first carried: "If the route is not
clearly smaller than what it replaces, cannot reach agreement on the widened
subset, or cannot prove two-value relations through its single certificate
route, the replacement stops and this record is withdrawn in favor of
migrating in place." [Route one measures](route_one_measures.md#verdict-against-the-stop-rule)
found that rule could not be evaluated. "Clearly smaller" had no number to
compare, because the route retired no old code. Agreement failed for
classified causes that the rule did not tell apart from a failure of the
design. Only the two-value clause could be decided, and it passed. A rule
that cannot fail cannot hold work back, and work went ahead
([below](#started-before-the-gate-paused-and-resumed)).

1. **The checking scope is the one the specification gives.**
   [Validation subjects](../../spec/build/validation_subjects.md) says a check
   "fully validates the packages selected as its subjects and checks the
   complete semantic dependency closure those declarations require, freshly or
   by exact-key reuse of a dependency slice. Everything else is outside the
   check's claim." The route's scope is neither
   of the two in use today:
   - *The route was too narrow.* `typed_trees_to_terminal_psi::elaborate` lowered
     only what the program entry calls. The specification's closure "starts
     from every declaration of every subject, including unused ones". `#2317`
     found 13 `fail/` programs the route accepted because the defect is in a
     machine the entry never calls (quoted in route one measures). That half
     is closed: `elaborate` takes the request's entries, what its Build
     selected and its subject package, makes every selected and every subject
     declaration a root, and refuses the compilation as `Unsupported` for one
     it cannot lower. A sweep found
     49 such `fail/` programs; none is accepted now, and the compiler test
     `lowered_route_subjects` pins it
     ([route one measures](route_one_measures.md#2-verdict-agreement)).
     Still open for this condition: the closure also names "the Build and
     preparation entries the compile executes". The route roots neither the
     Build program, which the Build activation checks with the current
     checker, nor the bound target's entry-contract source
     (`std/targets/<target>/entry.omg`), which the current checker keeps as
     a subject. The owner ruled on 2026-11-12 (`lowered-route-governance-scope`):
     while both checkers ship, the current checker judges the Build program
     and the entry-contract source in the same compilation, the route judges
     everything else. A compilation takes the route's verdict only when the
     current checker accepts the Build program and entry-contract source and
     the route accepts everything else; a rejection by either rejects the
     compilation. The split ends when the route can lower Build programs.
     The router implements this in the default compile
     (`products/src/terminal_publication.rs`, `ProductRoster::judge`): when the current
     checker finds nothing wrong in the Build program or an entry-contract
     source, a route rejection fails the compilation with the route's
     diagnostics (the current checker's follow when it rejects too), and a
     route acceptance of a program the current checker rejects elsewhere
     fails with "no product route yet", because provider selection and
     target preparation still need the current checker's output. A fallback, an `Incomplete` verdict or a
     defect of the route leaves the current verdict. The four entry-contract
     programs below therefore stay rejected by the current checker.
     `AcceptedClosure::publish` (`#2666`) publishes exactly the module the
     route checked as canonical Terminal Psi with its evidence seals. Each
     machine states its declaration identity from the typed trees
     (`typed-trees-to-terminal-psi`, `declare.rs`); its contract commitment is the
     route's own digest of the lowered contract, distinct from the old
     checker's digest. A stage-04 generated machine without a declaration
     identity still prevents publication.
     The 2026-11-13 product reconciliation retains the frozen typed owner
     before judgment with the shared `AcceptedTerminalSource` factory. It
     keeps the first closure only after every compilation part accepts and
     produces the single `PortableTerminalProgram` carrier. Publication
     failure refuses with "no product route yet"; it cannot select an old
     artifact. Entry receipts, composition roots, provenance and provider
     proposals use that published artifact. Native root, callback and
     component work keep their own artifact/evidence pair, validate the full
     bundle including dormant seals, then select the machine scope. Memory
     evidence never enters artifact bytes. Selected Psi optimizations with
     no evidence transport, named inspection and separately selected test
     roots without their own accepted source explicitly refuse.
     The outer product code now consumes frozen source preparation directly,
     without `CheckRequest`, the old checked-tree subject projection,
     `ProductSelections` or retained old judgments. Its coordinated Psi 05
     deletion and exact source-custody prerequisites are still being
     integrated; current-head default compilation and native/Psi agreement
     have not been demonstrated. Remaining checking-only consumers are cut
     or refused, not supplied with synthetic checked trees.
     Historical proof from the original product trial, at `1365094ea450`,
     ran `run/calls/scalar_contract_route_exit` on `windows_x86_64` with
     native exit 0 and Psi exit 0. A `linux_x86_64` image was produced on a
     still earlier recorded source. These are not results for the reconciled
     head.
     That default path retained a six-machine module without float rows.
     A separate Check-mode proposal trial selected `NativeFloats` rows and
     refused their absent external conformances. The actual default
     retained provider demand, rather than an all-catalog proposal request,
     determines the source metadata still needed by this consumer.
     `Build::validate` rows are not handled either; nothing implements them.
     The `fail/` sweep of 2026-11-12 found four programs
     the current checker rejected for their entry that were accepted whole by the
     route, because nothing it roots states what the target asks of an
     entry. They are `fail/build/program_entry_returns_value` and
     `fail/build/unrequested_target_entry_returns_value` ("`ProgramEntry`
     has no result"), `fail/build/hosted_program_entry_visible_parameter`
     ("hosted `ProgramEntry` exposes no arrival parameters") and
     `fail/build/uefi_program_entry_missing_storage_roots` ("exposes exactly
     image and initial-storage roots"). A rule in the route that an entry
     takes and returns nothing would close three and wrongly accept the
     fourth. The compilation now retains each executed binding's exact semantic
     requirement and target-declared visible parameter projection. Shared callable
     shape comparison rejects these four controls before body elaboration, including
     a binding outside the requested target. Matching a shape introduces no backing
     or `Granted` fact and does not prove the installed continuation's logical
     preconditions. Those obligations remain open; physical entry-source and
     package admission retain their existing owner. A fifth program of that sweep,
     `fail/build/executed_dependency_private_call`, was accepted for another
     reason, a call into a dependency's private machine, and is now handed
     back (`lowered_route_subjects`,
     `a_call_into_another_packages_private_machine_is_never_accepted`).
   - *The current pipeline is too wide*, and this record must not copy it:
     see [the library correction](#the-library-is-a-dependency-not-a-subject).

   Passes when, for the gate's scope, the declarations the route checks are
   exactly every declaration of the subject package plus what those
   declarations and the Build's selections demand; an unused subject
   declaration with an invalid body is rejected; an unused subject declaration
   that depends on an invalid dependency declaration is rejected; and a
   dependency declaration nothing demands is not checked and supplies no fact.

2. **A wrong obligation list is caught, and tests show it.** Elaboration
   writes both the lowered program and the list of obligations that program
   owes ("mints every obligation", `typed-trees-to-terminal-psi` README). The prover
   then proves what is on the list. If elaboration leaves a bounds check off
   the list, or states it about the wrong value, every obligation on the list
   is still proved and the program is accepted. The decision's second step
   says obligations are reconstructed from the module; today they are read
   from the list the same code wrote. The existing test
   `every_obligation_has_exactly_one_verdict` checks that each listed
   obligation gets a verdict, not that the list is the right one.

   Passes when a check that shares no code with the minting step derives what
   each operation, call, return and construction owes from the lowered
   module itself, and tests show it refuses each of these: an obligation
   removed from the list, an obligation whose goal was weakened, and an
   obligation attached to a different operation or value than the one that
   owes it. Each test changes only the list and leaves the module alone.

3. **One generic body goes the whole way.** The route has no generic code:
   a generic is refused as `Unsupported`. How a generic body is checked is
   still an [open question](#open-questions), and the answer on reusing proofs
   depends on a step that "is not built yet". The gate asks for one body, not
   the generics lane.

   Passes when one generic machine is checked once as written, against its
   declared requirements and with its parameters left abstract; is specialized
   for one concrete selection; that specialization is checked again, with the
   consumer working out its own goals and `proof/admission` deciding the
   carried certificates against them without a new search; and the resulting
   program passes `omega run --both` with exit 0. Its negative twin, which
   differs only in the generic's own contract or argument, is rejected.
   The re-check mechanism (`reverify`) landed with the first result below.

   <a id="condition-3-first-result"></a>
   **First result, 2026-11-11.** One generic machine,
   `advance<Selected satisfies Step>`, is lowered once with `Selected` left
   abstract and proved against `Step::apply`'s contract. Its certificates are
   then decided again by `proof/admission` in each of two generated
   specializations, against goals and hypotheses the specialization computes
   from its own lowered module, with no search
   (`terminal_psi_to_terminal_psi::reverify_specialization`). The program,
   `run/generics/implementation_parameter_contract`, passes
   `omega run --both` with exit 0, and its fail twin, which differs only in
   the bound `advance` promises, is rejected. What this does not yet show:
   - The parameter is an implementation parameter. A type or value parameter
     is not served, and the check tables have no row for a value of an
     abstract type
     ([what the lowered form does not carry](../../../omega-rust/psi/compilation/pipeline/06_terminal-psi-to-terminal-psi/README.md#what-the-lowered-form-does-not-carry)); [lowered type generics](lowered_type_generics.md) proposes how it
     could be.
   - The route was not the compile path when this was measured. `omega run
     --both` runs the program through the current pipeline; the generic check
     and the carried certificates run in the `generic_certificate_carry`
     example on the same typed trees. The default compile now judges the
     generic forms (below, "In the default compile").
   - Whether this meets the condition is the owner's reading.

   <a id="condition-3-value-parameters"></a>
   **Second result, 2026-11-12: value parameters.** A machine over `const
   Units: u64` is lowered once with `Units` a formal that no caller passes,
   and proved from its own `requires`. The machine stage 04 generates for
   `<7>` then has each certificate decided again by `proof/admission` with
   the literal in the formal's place, against its own goals and hypotheses
   and with no search. The witness is
   `pass/facts/lexical_generic_bound_through_local_exit`, the first in
   [lowered type generics](lowered_type_generics.md#recommendation-and-order):
   both of its generic bodies, `Selected::value<const Units: u64>` and
   `selected<Implementation satisfies Bounded>`, are checked once and
   carried, and its fail twin, which differs only in the trait member's
   `requires`, is refused at the member's refinement proof while a search
   over the generated caller alone accepts it. The compiler test
   `lowered_route_generics` pins both, with `#2378`'s pair. The program
   passes `omega run --both` on the current pipeline (native exit 0, Psi
   exit 0, Linux x86_64, `omega` built at `b741666a1`). Limits, each stated with its reason in the
   [terminal-psi-to-terminal-psi README](../../../omega-rust/psi/compilation/pipeline/06_terminal-psi-to-terminal-psi/README.md#value-parameters):
   a body that reads the parameter as an operand is not carried; a member's
   refinement of a requirement with value parameters is proved per argument
   in use, not for every value; a value parameter in a type waits for
   arrays. The route is still not the compile path.

   <a id="condition-3-type-parameters"></a>
   **Third result, 2026-11-12: an opaque type parameter.** A machine over
   `T` is lowered once with each value of type `T` rigid: it has an
   identity, a call passes it, and no proposition may name it. The machine
   stage 04 generates for a type then has each certificate decided again by
   `proof/admission`, unchanged, once the carrier has checked that every
   `T` value is a value of one concrete type there. The pair written for
   it is `run/generics/type_parameter_contract`, where `relay<T>` is proved
   against the contract of `forward<T>`, which it calls at its own `T`, and
   both are carried into the machines for `u8` and for `bool`; it passes
   `omega run --both` on the current pipeline (native exit 0, Psi exit 0,
   Linux x86_64, `omega` built at `b741666a1`). Its fail twin,
   `fail/generics/type_parameter_contract_exceeds_callee`, differs only in
   the bound `relay` promises and is refused once, as a generic body. The
   current checker reports it three times, for `relay` and for each of the
   two generated machines. The
   design's own witness, `pass/facts/generic_comparison_body_through_match_join_exit`,
   lowers and corresponds, and proves nothing: its two generic machines own
   no contract clause, its `main` uses a scalar `match` the route does not
   lower, and what tells it from its fail twin is what a caller learns from
   a callee's body, which the decision `callee-result-body-summary`
   ([state contracts](../../spec/language/state_contracts.md)) settles. Limits, in the
   [terminal-psi-to-terminal-psi README](../../../omega-rust/psi/compilation/pipeline/06_terminal-psi-to-terminal-psi/README.md#type-parameters):
   only where `T` is a scalar type; no operation has a result of type `T`;
   a `T` value is used at most once unless `T` is `[copy]`, because the
   route has no judgment of moves.

   **Fourth result, 2026-11-12: a finite type family.** Under the owner's
   ruling that checking a finite family once per listed alternative
   satisfies "every admitted alternative" (generics.md, "Static type
   equality"), `pick<T>` with `requires T == u8 || T == u16` is checked
   with `T` fixed to `u8` and to `u16`, each alternative's certificates are
   carried into the machine stage 04 generated for it, and the kernel
   decides them there. The pair is `run/generics/finite_family_contract`
   (`omega run --both`: native exit 0, Psi exit 0) and its twin
   `fail/generics/finite_family_contract_wrong_alternative`, refused at the
   `u16` alternative. Limits, in the
   [terminal-psi-to-terminal-psi README](../../../omega-rust/psi/compilation/pipeline/06_terminal-psi-to-terminal-psi/README.md#finite-families):
   one type parameter listed by one clause of builtin scalar types.

   **In the default compile, 2026-11-12.** Since `#2674` a subject's generic
   machines no longer hand the compilation back. The router judges the
   closure of the compilation's roots and, beside it, each generic machine
   as its own check form (`terminal_psi_to_terminal_psi::judge_compilation`): once with
   its parameters rigid, or once per alternative of a finite family, whether
   or not the program applies it. A compilation is decided only when every
   part is. An unused `bump<T>` over `u8 | u16` whose body can overflow is
   rejected for both alternatives, which the current checker never checks
   (`lowered_route_subjects`). Still open: the machines stage 04 generated
   for the program's own applications are checked again by search as closed
   machines; carrying the generic form's certificates into them
   (`reverify_specialization`) is not wired into the compile.

4. **For one fully supported scope, old code is retired and the result is
   measured.** A scope is fully supported when every program in it is governed
   by the route with no fallback: nothing in the program or in the closure it
   demands returns `RefusalReason::Unsupported`. Route one measures found
   that the route "retires none of Psi 05, validation or Psi 06".

   Passes when, for one such scope, the code in those three stages that only
   that scope needed is deleted on `main`, the scope's corpus rows are still
   green with the old route unable to serve them, and the record states,
   from the landed diffs, the lines deleted, the lines the route needed for
   that scope, and check time before and after. The gate sets no ratio in
   advance: the old rule's "clearly smaller" failed because it named no
   number. Both numbers are reported side by side, and if the route's lines
   exceed the lines retired, the record says so beside the result.

   **Measured 2026-11-12: the covered subset makes almost none of the old
   checker unreachable, so nothing is retired.** The 12 programs of the
   [places gate](places_foundation_lane.md#gate-re-run-2026-11-12) that both
   checkers accept run 233,226 of the 596,993 instrumented lines of the
   three stages. A sample of 36 programs the route does not serve (every
   fourteenth of the gate's 502 "outside" programs) runs 325,931. Together
   they run 326,596, so 665 lines are run by the accepted programs and by
   none of the sample: 200 in Psi 05, 305 in validation, 160 in Psi 06.
   The other 232,561 lines the accepted programs run are also run by
   programs the route cannot check.

   | Lines | Psi 05 | validation | Psi 06 | All three |
   | --- | ---: | ---: | ---: | ---: |
   | Instrumented | 247,292 | 254,856 | 94,845 | 596,993 |
   | Run by the 12 accepted programs | 97,946 | 98,866 | 36,414 | 233,226 |
   | Run by the 36 unserved programs | 133,953 | 140,192 | 51,786 | 325,931 |
   | Run by either | 134,153 | 140,497 | 51,946 | 326,596 |
   | Run only by the accepted programs | 200 | 305 | 160 | 665 |

   What this shows is that the old stages are not divided by construct in
   the way the route's subset is: a scalar call with a contract goes through
   the same declaration, custody, planning and lowering code as a program
   with borrows or services, so serving the scalar subset frees no code.
   Retirement by scope will come from serving whole programs of a kind the
   old stages treat separately, not from the subset growing by construct.
   The 665 lines are an upper bound from a sample of 36, not a list of dead
   code; a wider sample can only lower it. Nothing was deleted: when this
   was measured the default compile had no router, and every one of these
   lines is still needed by a compile the route does not govern.

   Limits. Lines are those `cargo xtask stage-coverage` instruments in
   `05_typed-trees-to-checked-trees/`, `validation/src/` and
   `06_checked-trees-to-lowered-psi/`, run by `omega --check` on Linux
   x86_64 with the compiler built from the worktree at `1726ca7b560d` plus
   uncommitted route changes, which touch none of the three stages. 14 of
   the 36 sampled programs exited 1 under `omega --check`; the one inspected
   (`pass/constants/runtime_scoped_const_exit`) stopped on a program-entry
   binding that `omega update` has not accepted, which the corpus runner
   supplies and the bare command does not. The lines they ran before
   stopping are counted. The 12 accepted programs include none with a
   generic body.

The conditions are deliberately small: one scope, one generic body, three
kinds of wrong list. Passing them does not show the replacement will finish.
It shows the four things that had been assumed. Agreement between the two
checkers stays a measure and is reported as before; it is not a gate
condition, because a disagreement can be a prover gap, an error in the
current checker or an open question, and only reading tells which.

Withdrawing this record in favor of migrating in place remains the owner's
decision. The gate sets no deadline and no automatic withdrawal.

#### One meaning per obligation

The measure first read "routes that can prove one kind of obligation (one; a
call's `requires` has about twenty today, tried in order)". That wording
counted the wrong thing. What must be one is the *meaning* of an obligation:
what has to hold, stated once from the operation that owes it, and the same
whether the value it speaks about is a local, a field, a parameter, a call
result, the result of a join, a named constant, or a value in another state,
another machine or a Build-selected body. How the obligation gets proved may
vary. Several proof methods are allowed behind one meaning, as long as each
ends in evidence the proof kernel accepts.

So the measure is two counts per obligation kind, each with target one:
definitions of the goal, and ways an obligation can be accepted (the kernel
deciding a certificate). The prover's five methods (a known fact, a closed
relation, bounds and order chains, affine arithmetic, one case split) are
five methods behind one meaning, and adding a sixth does not break the
measure. The 27 routes the current checker tries for a call's `requires`
fail it for a different reason than their number: none emits evidence, and
by their names several are chosen by where the value comes from
(`parameter_domain_grants`, `subslice_grants_domain`,
`value_call_return_domain_grants`), so the same requirement can be decided
differently in different places.

#### The library is a dependency, not a subject

Route one measures concluded that "every program, the one-line ones
included, runs 45% of those stages' instrumented lines to check the bundled
library", and offered keeping the route "until the library enters the
subset". The measurement is right about what the current pipeline does. It
is not what the specification requires, and the conclusion drawn from it is
withdrawn.

[Validation subjects](../../spec/build/validation_subjects.md): "Being
bundled with the toolchain, vendored, located in the workspace or editable
does not make a package a subject. A dependency is a dependency." A program
that is checked or built is the subject. The bundled library enters its
check only through the closure its declarations and Build selections demand.
`#2257` (open draft, quoted, not re-measured) counted a one-line program
running the proof stage on 837 machines and reaching 8.

Two consequences for the gate:

- The size comparison in condition 4 is made on the scope of condition 1.
  A scope can be fully supported without the whole library being supported:
  it needs only the library declarations its programs demand.
- The whole library is still checked in two cases, and neither is "every
  program". A library package is checked in full when it is itself a
  subject, and "the toolchain's own release validation checks each bundled
  library as a subject". So the three old stages cannot be deleted outright
  until the route supports the library as a subject. Retiring the code one
  scope needed, which condition 4 asks for, does not wait for that.

A consumer checks every dependency declaration it needs, freshly or by
exact-key reuse of a checked dependency slice (owner ruling 2026-11-12,
[validation subjects](../../spec/build/validation_subjects.md#what-this-rule-does-not-do)).

#### Started before the gate, paused, and resumed

The plan below said the lanes after route one would start once its gate
passed. Two started before any gate was evaluated. This is the state on
`main` at `c7371d3dc` and in the open pull requests on 2026-11-11.

| Lane | State | What exists |
| --- | --- | --- |
| Places (records, fields, cases, patterns, data `requires`) | started, on `main` | `#2328` split `typed-trees-to-terminal-psi` into per-family modules. `#2332` records and field reads, `#2337` field stores, whole assignment and locals without an initializer, `#2343` sums, case dispatch and patterns, `#2356` payload-binding justification, `#2362` data `requires` at construction and every store. |
| Numbers (integers, floats, casts and domains) | started, not on `main` | `#2365` (open draft): result bounds for divide, remainder, shift and bitwise operations, described as that lane's first slice. It reports 4 of the 12 prover gaps in route one measures now agreeing (quoted). |
| Loops, recursion and termination | resumed 2026-11-12 (paused until then) | A staging plan on branch `work/docs-loops-recursion-termination-lane` (`d352f7e36`). No pull request is open for it and no code exists. |
| Proofs, evidence and quotients | resumed 2026-11-12 (paused until then) | `#2366` (open draft, marked do not land): erased scalar bindings, the first of eleven planned slices, with the lane's plan file. |
| Every other lane | not started | |

Also open from route one itself: `#2317` (join predicates and crash
coverage, draft) and `#2309` (re-verifying a saved proof, tests only).
`#2321` landed the verdict a router would take from the route; the router
that takes it is described under [condition 1](#the-gate).

The three route crates were 9,463 lines of Rust at `82d2ea5d5`, where route
one measures counted them, and are 21,576 at `c7371d3dc`, tests included
(`typed-trees-to-terminal-psi` 11,082, `terminal-psi-to-terminal-psi` 9,392,
`terminal-check-tables` 1,102). None of the six places commits on `main`
touches a file in Psi 05, validation or Psi 06 (`git show --stat`).

The owner lifted the pause on 2026-11-12. Until then no construct family was
added to the route and the loops and proofs lanes stayed where they stopped.
Now every lane resumes, and the gate conditions are pursued alongside the
lanes rather than before them. The places work that landed stays, and closing
a case where the route accepts an invalid program still goes first.

### Lane order

The owner ruled on 2026-11-12 that this work outranks every other compiler
item, including the `TASKS.md` priority migrations: a construct a lane needs
is built in the lowered-form checker, not in the stages it replaces. By the
later ruling of the same day those stages are frozen: no pull request edits
them except to delete them
(`LOWERED-FORM-CHECKER` at the top of [the task board](../../../TASKS.md)).

Since the owner lifted the pause on 2026-11-12, the remaining construct
families run alongside [the gate](#the-gate)'s conditions, in two
dependency-ordered groups rather than ten simultaneous sessions. The
split of `typed-trees-to-terminal-psi` into per-family modules and part of the
places foundation have already landed
([above](#started-before-the-gate-paused-and-resumed)):

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

There is no second checker to agree with. A lane is measured by the programs
it moves from refused to judged: the router refuses a compilation on any
`RefusalReason::Unsupported`, so the count of refused programs in the
post-merge ledger is the progress number, and a lane is done when the corpus
rows of its families are judged and their verdicts match the rows' expected
outcomes. The first lane by programs unblocked is the product hand-off from
the route's published module, then Build programs and the entry-contract
source, which every compilation needs.

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
- Omega 01 turns every operation obligation reference in a unit into an
  accepted fact, with no evidence attached
  (`01_abstract-operations-to-abstract-operations/src/verified_unit.rs`
  `source_checked_obligation_facts`); the interpreter still checks. A wrongly
  kept fact therefore makes native code and the interpreter disagree. An
  earlier wording said the back end read a list of source-checked obligations
  from lowering; no reader of that list was found.

The full read added seven places that accept on missing or mismatched input.
Each was traced on 2026-11-10 at `c118c914794`, and the probes were run on the
Mac mini. One accepted an invalid program and has since been closed; the other
six are closed or not
reached:

- **Accepted an invalid program; closed.** The default-domain driver skipped
  its end-of-state window check when the state contained any crash statement,
  even one that cannot run (`domains_and_predicates.rs` `walk_state`). A write
  that breaks a data `requires`, then a complete dispatch whose arms all
  return, then a dead `crash Trap;` was accepted. Without the crash, the same
  program was rejected. The driver now checks open windows at each returning
  row of the dispatch and lets only a crash row that ends the dispatch abandon
  one. Witness: `fail/dependent/data_requires_return_beside_dead_crash`, now
  rejected with a recorded ledger row. A crash beside a normal exit on another
  path cannot be written: the parser makes `crash` an unguarded statement that
  ends the dispatch.
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
  fallback only if that fails. Condition 3 of [the gate](#the-gate) runs this
  test on one body before the lane exists, and
  [its first result](#condition-3-first-result) is for an implementation
  parameter only.
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
  `StructuralFieldId`, and `typed-trees-to-terminal-psi` classifies builtin scalars
  through `SymbolTable::builtin_type_atom` on the resolved root symbol rather
  than by spelling.
- **Diagnostics.** Errors must name source constructs. `LoweredSourceCustody`
  covers call occurrences only and depends on `checked-trees`, so `#2286`
  carries per-machine `MachineSources` over dense identities with explicit
  `Unpositioned` reasons; whether that suffices for every diagnostic across the
  remaining families is checked as each lane lands. Measured on every
  `fail/` fixture: [route one diagnostics](route_one_diagnostics.md). Where
  the route rejects, 288 of 293 rejections name the defect's obligation at
  its construct, but no message states the failing condition; the missing
  part is mapping, not information the lowered form lacks.
- **Size.** That the replacement is far smaller than 792,000 lines is the
  premise of this record and is unmeasured. Condition 4 of
  [the gate](#the-gate) is the first measurement of it, on one scope.
- **Reuse of checked results. Answered 2026-11-11 (Jarod), decision
  `specialization-certificate-reuse`:** a proof that was found once is carried
  forward, and every consumer checks it again. The consumer works out its own
  goal from its own program and has `proof/admission` decide the carried
  certificate against that goal, without searching again. A certificate that
  no longer fits is refused, and outside exact-key reuse of a dependency
  slice a pass from an earlier compile is never used in place of checking. This is how the two clauses are read together:
  [`generics.md`](../../spec/language/generics.md#closed-applications-and-canonical-identity)
  ("Carry checked derivations through specialization and published artifacts
  without re-searching") is about search, and
  [`validation_subjects.md`](../../spec/build/validation_subjects.md#what-this-rule-does-not-do)
  (a consumer checks every dependency declaration it needs; since the owner
  ruling of 2026-11-12 that check may be exact-key reuse of a dependency
  slice's checked result) is about checking. `terminal_psi_to_terminal_psi::reverify` is the
  tested mechanism, and `reverify_specialization` carries a generic body's
  certificates into a specialization that selects an implementation;
  substituting a concrete type for a type parameter in a certificate is not
  built yet.

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
  code is, not that the new checker will be smaller. The stop rule this
  bullet relied on has since been replaced by [the gate](#the-gate).

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
