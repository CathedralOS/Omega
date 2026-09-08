# Documentation consolidation

Temporary execution plan for
[DOCUMENTATION-CONSOLIDATION](../../TASKS.md#documentation-consolidation).
Delete it when the finish conditions hold.

## Required result

A reader can find how to use Omega, what it guarantees, what is proposed, and
what remains unfinished. Each current rule has one authoritative home.
Preserve useful contracts and explanations, not the old documents' volume.

| Home | Content |
| --- | --- |
| `wiki/language_guide/` | Teaching and worked examples. |
| `wiki/spec/` | Current language and public toolchain contracts. |
| `wiki/proposals/` | Concrete proposed changes and concise useful decision rationale. |
| `wiki/drafts/` | Temporary investigations and migration plans, deleted when finished. |

Implementation ownership belongs beside code. Keep a short documentation index,
not another architecture hierarchy. Moving a document without reviewing and
consolidating its contents does not complete this work.

## Specification style

Write useful reference material, not a narrative of the design process.
Use grammar for syntax, definitions and rules for meaning, tables for closed
alternatives, and equations or formal judgments when they improve precision.
Keep prose that states conditions, consequences, or necessary distinctions.
Examples clarify rules; they do not replace them.

Remove repeated explanations, implementation milestones, file-size reports,
version-bump histories, and speculative alternatives from current contracts.
Put design arguments in proposals and teaching in the guide. Do not call prose
a formal specification or present intended behavior as implemented support.

## Proposal process

Borrow the compact motivation/design/alternatives structure of
[OCaml RFCs](https://github.com/ocaml/RFCs), the proportional process of
[Go proposals](https://go.googlesource.com/proposal/+/master/README.md), and the
subject-organized reading model of the [Go specification](https://go.dev/ref/spec).
Keep the name **proposals** and keep them in this repository.

A proposal needs a descriptive title, open or accepted status, affected subjects,
the problem, a concrete design, useful alternatives, and unresolved questions.
Do not require a proposal for routine engineering or a separate permanent
document for every owner answer. No registry, committee, or new tooling is needed.

Acceptance updates the specification and guide. Missing implementation gets one
owning task. Keep an accepted proposal only when its concise, explicitly
nonnormative rationale remains useful; remove duplicate rules and discussion
transcripts. Rejected designs normally leave the tree. Git retains them.

## Execution

### 1. Consolidate the large mixed references

Audit remaining guide and draft material against the existing specification and
implementation owners before adding another document. Prioritize large duplicate
blocks and historical implementation accounts over small editorial changes.

For each coherent subject:

1. Determine the intended current contract, resolving obvious stale text.
2. Put the contract in the spec and implementation details beside code.
3. Retain only real unfinished work in its existing task or a temporary draft.
4. Delete the displaced source text and repair links and source readers in the
   same checkpoint.

Split by subject, not by decision number or patch. Do not preserve two complete
versions while waiting for a future deduplication pass. Genuine unresolved
semantic conflicts go to `OWNER_QUESTIONS.md`, with an explicit undetermined
state in the ported subject. Unrelated subjects can proceed.

### 2. Make the guide teach

Extract its normative rules into their specification subjects while preserving
examples and explanations. Remove implementation diaries, old source forms,
and decision numbers as required reading. Mark real support limitations and
link their task; do not weaken the language to match the current compiler.

### 3. Remove obsolete residue

Port or delete every remaining document. Preserve release evidence only where
an identified consumer needs it; do not confuse evidence with a changelog.
Move tool usage beside its tool and completion plans to drafts or tasks.
Do not create a permanent archive of obsolete designs.
Useful project-specific notes without a permanent owner, such as Cathedral
alignment, move to drafts after review. They need not become language documents
or formal proposals. Give them a purpose and removal condition; delete stale
claims and redundant history rather than moving them unchanged.

Bootstrap contracts follow the same rule: preserve current requirements in
[their owners](../../bootstrap/CONTRACT.md), not decision numbers or superseded
history. The
[chain comparison proposal](../proposals/bootstrap_chain_alternatives.md) is
separate from the selected chain and its
[minimization contract](../../bootstrap/MINIMIZATION.md).

### 4. Close the migration

Update navigation, instructions, skills, task links, and documentation readers.
Use existing boards, not a new status database. Remove completed tasks and drafts.
Maintain current rules in place; Git owns implementation history.

## Finish conditions

- Every current subject has one authoritative owner and a useful entry point.
- The guide teaches; proposals and drafts do not compete with the specification.
- `pre_migration/` is empty and removed, including the bootstrap records.
- No completed-work diary remains in current reference material or task boards.
- Local links and anchors resolve, displaced paths have no stale readers, and
  affected source-reading tests pass. Documentation edits alone do not require
  a full compiler rebuild.
- Contracts are preserved, unspecified behavior is explicit, and implementation
  or formal-verification status is not overstated.

Next: consolidate the guide's ownership, concurrency, cleanup, and package/module
chapters against their specification owners. Then audit remaining
historical references, drafts, navigation, and source readers. Complete byte-level
Terminal tables remain explicit PSIIR work, not an implied achievement of prose
migration.
