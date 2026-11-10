# Check on the lowered form

Architecture decision, 2026-11-10: how the compiler is built. Not a language
change: the specification is untouched. Evidence was read and counted on `main`
at `042971d0288`; nothing here was built or run, and no fixture outcome is
claimed.

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

Psi 00 through 04 and the Omega stages are unchanged. Psi 05, validation and
Psi 06 are replaced. The current pipeline stays the shipping path until the
replacement serves a construct family completely.

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

## Route one, before anything else is built on it

[Compositional lowering](../../../omega-rust/pipeline.md#compositional-lowering)
requires "one working, independently checked route through the new mechanism"
before variants are added. Route one is:

- **Subset.** Machines whose bodies use scalar locals, assignment, builtin
  arithmetic and comparison, direct calls with `requires` and `ensures`,
  `transition` arms and returns. No references, records, loops or generics.
- **Build.** Structure-only elaboration of that subset to unsealed Terminal
  Psi; obligation reconstruction; discharge by the existing arithmetic engine
  with its hypotheses read from Terminal values instead of source shapes.
- **Oracle.** Every corpus program inside the subset is checked by both
  routes. A disagreement is a defect in one of them and is classified, not
  averaged. The must-reject programs listed below guard the cases where the
  current checker is itself wrong.
- **Measures.** Lines of code for the route against the code it would retire;
  verdict agreement; check time per program; lines added per row when one more
  construct is added.
- **Stop rule.** If the route is not clearly smaller than what it replaces,
  or cannot reach agreement on the subset, the replacement stops and this
  record is withdrawn in favor of migrating in place.

Only after route one passes do further construct families move: records and
fields; references, loans and windows; loops and recursive components;
runtime callables; generics.

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
- A checker verdict marks a lowered obligation source-checked, and Omega 00
  accepts it; the interpreter still checks. A wrongly kept fact therefore
  makes native code and the interpreter disagree.

Must-reject programs the corpus lacks, none under `facts/`: knowledge that
survives a pause; a write through a partly overlapping window; a linear value
consumed on a loop back-edge; a copied scalar whose source a later operand
overwrites; a rank premise across a pause.

## Open questions

- **Generic bodies.** Terminal Psi is "concrete, and post-instantiation", and
  the specification requires a generic body to be valid for every admitted
  selection. Either the checked form admits unresolved binders, or templates
  keep a tree-level check. Unresolved.
- **Evidence coverage.** Validation's automation "does not emit kernel
  evidence for every accepted obligation". How much of it can emit evidence
  the admission kernel accepts is not measured.
- **Diagnostics.** Errors must name source constructs. `LoweredSourceCustody`
  carries source-to-Terminal correspondence today; whether it is enough for
  every diagnostic is not checked.
- **Size.** That the replacement is far smaller than 792,000 lines is the
  premise of this record and is unmeasured until route one.
- **Reuse of checked results.** `generics.md` requires checked derivations to
  be carried through specialization; `validation_subjects.md` says "No result
  is reused". That is an owner question.

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
