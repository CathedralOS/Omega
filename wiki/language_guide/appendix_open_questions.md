# Appendix: Open Questions

This appendix lists unresolved language-design areas only. Settled behavior
belongs in the relevant chapter or frozen design brief, not here. Immediate
owner decisions belong in the repository-root `OWNER_QUESTIONS.md`;
engineering work belongs in `TASKS.md`.

Last pruned: 2026-08-29.

## Reach, resources, and progress

- Extend opaque, sealed progress profiles into a general trace
  logic only when a concrete customer needs machine-side fairness, deadlines,
  starvation freedom, or profile entailment. `terminates`, service reach,
  operational clauses,
  and ungranted provider claims do not manufacture those theorems.
- Settle the quantitative resource algebra before adding entries such as
  `Alloc<Peak, Retained>` to reach rows, general owned-buffer splitting, or
  claiming compile-time reconciliation of local task-pool child leases.
- Defer additional operational clauses and named or ordinary-export service-row
  polymorphism until a concrete customer forces their declaration, coherence,
  and separate-compilation rules. Installation-bound provider requirements
  already admit one path-keyed bounded row that cannot escape its root closure.
- Define scheduler operation contracts in terms of sealed progress
  profiles, including wake-one/wake-all and timed-wait placement.

## Concurrency and hardware

- Decide which evidence-backed suspension-safe loans, if any, extend the current
  rejection-first conservative subset, and then whether borrow/wait-cycle
  detection earns a later proof mode. Whole-system deadlock reasoning is not a
  prerequisite for moved-ownership tasks.
- Determine how far the proof system should support disjoint mutable sharing
  for lock-free structures before requiring a mediated or accepted boundary.
- Define the formal atomic-event model and prove the existing x86-64/AArch64
  mappings before implementing the settled portable fences and protocol
  checker. Keep checked ISA, compiler-only fences, and device/DMA ordering
  operations distinct.

## Domains, proofs, and arithmetic

- Settle the source spelling for one joint ranking across a
  mutually cyclic SCC whose participants expose differently shaped subjects.
  The every-cyclic-edge-decreases semantics and private-witness identity are
  already fixed.
- Decide how much predicate/domain inference to attempt beyond executable
  predicate bodies, explicit evidence, and flow narrowing.
- Settle general open-family domain linking and `weakens_to` certificate syntax.
- Decide whether invariant windows may ever carry graph-edge proof debt; the
  current rule treats transitions as consumption points.
- Specify how weakened machine invariants appear in target-state signatures.
- Define checked-result arithmetic, if it earns a distinct library surface
  beyond exact-by-default obligations and explicit policies.
- Define how `Real` contracts are instantiated for `f32`/`f64`: explicit error
  bounds, inferred bounds, or named approximation policies.
- Extend sequence-wide text proofs beyond validate-once plus preservation
  lemmas when richer inductive invariants have a concrete customer.
- Expose proposition-expression implication, conjunction, and falsehood only
  when a source theorem needs to compose them as values of the proof logic.
  The proof kernel already checks those forms; ordinary `requires`/`ensures`
  clauses and Boolean facts cover the current proposition-family and decider
  customers without a second surface.

## Core surface and types

- Settle the boundary between browsable core declarations and
  compiler-managed primitive carriers. Current direction keeps `Array`, `Vec`,
  and `Slice` public; text is bytes plus an encoding domain.
- Settle explicit discriminants versus the first-case/tag-zero invariant,
  generic payload layout under stable representations, and the remaining
  generic case-union/exhaustiveness interactions. Ordinary transition-arm
  payload binding already uses the data-pattern rules in Chapter 1.
- Define foreign-type domain imports, orphan/coherence restrictions, and their
  authority-report representation.
- Decide whether any core generic properties beyond `copy`, `linear`, `sized`,
  and parameterized carry earn inclusion.

## Boundaries, assembly, and components

- Define the first per-target checked-instruction catalogs and their
  user/deriver-only availability. Opaque/manual raw assembly is not an
  alternative.
## Tooling and build-time execution

- Decide which contextual words, if any, must become globally reserved. The
  default remains to avoid new reserved words.
- Design an in-language test item/discovery surface; canaries remain external.
