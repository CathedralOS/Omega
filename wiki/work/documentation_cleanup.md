# Documentation consolidation

Execution plan, not a new language specification. Owner: repository documentation;
execution entry: [DOCUMENTATION-CONSOLIDATION](../../TASKS.md#documentation-consolidation).
Delete this plan when the finish conditions below hold. The
[documentation index](../README.md) names migrated subjects and the legacy
references that still own the rest; directory creation is not completion.

## Required outcome

A reader can find how to use Omega, what Omega guarantees, what is proposed,
and what work remains without reconstructing a sequence of owner conversations.
Each rule has one authoritative home. Guides explain it, proposals suggest changes,
and work notes track implementation. None independently redefine it.

This is consolidation and deletion, not a folder-renaming exercise. Preserve
current intended contracts and useful explanations, not the structure or volume
of the old documents. Rewrite a mixed document when that is simpler than moving
its paragraphs around. Git retains obsolete designs and completed work.

## Destination

Keep documentation in this repository and retain `wiki/`; changing that prefix
adds no useful separation. Its four main areas will be:

| Home | Purpose | Excludes |
| --- | --- | --- |
| `wiki/language_guide/` | Teaching, worked examples, practical use of the current language. | Compiler implementation diaries and competing normative rules. |
| `wiki/spec/` | Current language and public toolchain contracts, organized by subject. | Proposals, release progress, and accidental Rust implementation restrictions. |
| `wiki/proposals/` | Concrete proposed changes and selected concise decision rationale. | A second current specification or a mandatory proposal for routine engineering. |
| `wiki/work/` | Active investigations, migration plans, and completion criteria. | Permanent authority or completed status reports. |

Use one short `wiki/README.md` to explain these roles and provide entry links.
Within the specification, separate language semantics, portable artifacts and
verification, and build/package/publication contracts. Terminal Psi's public
format is a specification; its Rust arena organization is not. Bootstrap
language and execution contracts also need explicit specification owners, not
copies in several architecture and rung documents.

Call this the **Specification**, not the Formal Specification. Exact prose,
grammar, and tables are useful now. Formal judgments and checked definitions
can replace appropriate portions as they exist; do not imply that prose has
already been formalized or that the Rust implementation defines the language.
Mark genuinely unspecified points explicitly and route decisions through
`OWNER_QUESTIONS.md`; editorial consolidation cannot choose new semantics.

Implementation ownership and invariants belong beside their implementation,
with a short repository/compiler overview linking those local documents.
Tool usage belongs with the tool or its user documentation. These are local
supporting docs, not additional competing specifications. Keep cross-component
invariants at their lowest shared owner rather than duplicating them per crate.

## What to borrow from existing projects

- [OCaml RFCs](https://github.com/ocaml/RFCs) use a compact design outline:
  summary, motivation, technical design, drawbacks/alternatives, and open
  questions. Borrow that content, not its committee or separate repository.
- [Go's proposal process](https://go.googlesource.com/proposal/+/master/README.md)
  starts with a brief proposal and requests a detailed design only where needed.
  Borrow that proportionality: an owner question can resolve a small change
  without producing another permanent document.
- [Go's specification](https://go.dev/ref/spec) presents the language by current
  subject, with grammar and semantic rules together. Borrow that reading model,
  not its particular language organization or a requirement for one giant page.

No new proposal service, repository, committee, meeting cadence, numbering registry,
status database, or documentation build system is needed for this cleanup.

## Small proposal lifecycle

Use a descriptive filename and a title. At the top, state whether the proposal
is open or accepted and link the affected specification sections when they
exist. An open proposal contains context/customer, problem, proposed contract
with examples, viable and tempting-but-wrong alternatives, and remaining
questions. Omit sections that genuinely add nothing. Code locations are migration
evidence, not the reason a language design should exist.

Acceptance updates the authoritative specification and affected guide passages
in the same change. If implementation is missing, leave one owning execution
task; do not describe an accepted design as implemented. Retain an accepted proposal
only when its concise rationale remains useful, explicitly nonnormative and
linked to the current rule. Remove its discussion transcript, version-by-version
updates, and duplicate contract. A reader must never need it to determine the
current rule. Rejected or abandoned proposals normally leave the active tree;
Git preserves them. Keep a rejected alternative only where a concrete recurring
question justifies a short explanation.

`OWNER_QUESTIONS.md` remains the small decision inbox, not another proposal archive.
Link a substantive proposal there rather than duplicating it. Routine bug fixes,
editorial repairs, and implementations of settled rules need no proposal.

## Four execution moves

### 1. Replace the largest competing authorities

Start with these mixed documents, not cosmetic renames of small files:

- [Terminal Psi](../architecture/pipeline/terminal_psi.md).
- [Build and package model](../design_briefs/build_and_package_model.md).
- [Calling plans](../design_briefs/calling_plans.md).
- [Typed-to-checked stage](../architecture/pipeline/stages/typed_trees_to_checked_trees.md).

For each coherent subject, extract the intended current contract into `spec/`,
put implementation-specific explanations beside the owning code, move only live
work into its existing task or a temporary note, and delete the rest. Remove
the displaced source text and update inbound links in the same checkpoint.
Do not retain two full documents while waiting for a future deduplication pass.
Split a large specification by actual subject, not by review number or patch.

Publish the short documentation index with the first replacement. During the
transition it names the remaining authoritative legacy pages explicitly; a
new directory alone does not silently supersede them. Review the subject's
cross-references and conflicting statements before declaring its move complete.
Evidence of a real unresolved semantic conflict goes to the owner; unrelated
subjects can continue. Do not preserve a contradiction under two headings.

### 2. Make the guide teach the language

Drain normative rules into the corresponding spec subject as each chapter is
revised. Keep examples, explanation, and concise rationale; link the exact
reference rules instead of reproducing all validation and encoding detail.
An example may restate a rule to teach it, but does not become a second authority.
Remove decision numbers as required reading, old source forms, release counters,
and compiler-phase implementation reports. Keep a short, clearly labeled pointer
to support limitations where an example is not yet usable; the task owns the gap.
Do not hide missing implementation or weaken the intended language to fit it.

### 3. Empty the mixed-purpose directories

Finish draining `design_briefs/` and the oversized `architecture/` hierarchy:
current contracts to the spec, live proposals to `proposals/`, implementation ownership
beside code, and genuinely unfinished plans to work notes. Delete completed
plans and superseded alternatives rather than creating an archive directory.
Retain only the short repository overview needed to navigate implementation.

Split `releases/` by what the files actually do. The
[Rust completion contract](../releases/rust_compiler_completion_contract.md)
is a work acceptance document; [rollback instructions](../releases/optimizer_rollback.md)
are tooling documentation. Actual release/promotion evidence belongs with its
identified release or rule and must remain retrievable when a concrete consumer
depends on it. Do not confuse required evidence with a narrative changelog or
delete it merely because it describes a past event.

The [bootstrap decision record](../architecture/bootstrap_chain/decisions.md)
is explicitly protected by `AGENTS.md`. This plan does not authorize changing,
moving, deleting, or superseding its entries. The desired final shape is current
contracts in the spec, with historical rationale recoverable from Git or concise
proposals, not a live competing ledger. Obtain explicit owner authorization for that
record's authority transition before executing it. Track that bounded exception
in this task; it does not block cleaning unrelated documents.

### 4. Close the migration and prevent renewed accumulation

Update README, `AGENTS.md`, skills, task links, and documentation readers to point
at the actual new owners. Remove compatibility copies and obsolete navigation.
Keep the documentation maintenance rule short: edit current rules in place;
implementation history goes in Git; temporary notes name their exit condition.
Use the existing boards, with one task per real unfinished outcome, not one task
per page or a new documentation status tracker. Delete this plan and its task
when done; do not turn them into a completion ledger.

## Checkpoints and finish conditions

Each checkpoint must remove a competing owner, simplify a guide chapter, or
retire mixed-purpose material. New folders and moved bytes alone do not count.
Report which old authority disappeared and where its remaining content lives,
not how many pages were touched. No preliminary repository-wide inventory or
template rollout is required before the first substantive replacement.

The cleanup is complete when:

- The index exposes the four roles; each normative subject has one current home.
- Guide, proposal, work, and local implementation docs do not compete with that home.
- Legacy design/architecture/release dumping grounds have been drained, including
  the protected-record disposition after explicit owner authorization.
- No completed migration or review-version diary remains on an active board or
  in current reference material; retained rationale and release evidence have a
  specific purpose.
- Local links and anchors resolve, displaced paths have no stale readers, and
  affected source-reading tests have been updated and run. Prose-only subjects
  require document checks, not a gratuitous full compiler rebuild.
- The result preserves intended contracts, distinguishes unimplemented from
  unspecified behavior, and makes no unsupported formal-verification claim.

Next executable step: consolidate the remaining Terminal operation and proof
reference into subject owners, deleting displaced
text and repairing its links in the same checkpoint. Do not start another
planning pass before that replacement.
