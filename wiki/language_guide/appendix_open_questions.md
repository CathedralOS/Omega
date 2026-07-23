# Appendix: Open Questions

This appendix lists unresolved language-design areas only. Settled behavior
belongs in the relevant chapter or frozen design brief, not here. Immediate
owner decisions belong in the repository-root `OWNER_QUESTIONS.md`;
engineering work belongs in `TASKS.md`.

Last pruned: 2026-07-22.

## Effects, resources, and progress

- Extend decision 23's opaque, sealed progress profiles into a general trace
  logic only when a concrete customer needs machine-side fairness, deadlines,
  starvation freedom, or profile entailment. `terminates`, effect-row members,
  and ungranted provider claims do not manufacture those theorems.
- Settle the quantitative resource algebra before adding entries such as
  `Alloc<Peak, Retained>` to effect rows, general owned-buffer splitting, or
  claiming compile-time reconciliation of local task-pool child leases.
- Defer user-defined operational effect members and effect-row polymorphism
  until a concrete customer forces their declaration, coherence, and
  separate-compilation rules.
- Finish the suspension implementation brief: continuation layout and capacity,
  cancellation behavior, and suspension-safe loans. Ordinary calls compose
  suspension without a call-site marker.
- Define scheduler operation contracts in terms of decision 23's sealed
  profiles, including wake-one/wake-all and timed-wait placement.

## Concurrency and hardware

- Decide the first conservative suspension-safe-loan subset and then whether
  borrow/wait-cycle detection earns a later proof mode. Whole-system deadlock
  reasoning is not a prerequisite for moved-ownership task v1.
- Determine how far the proof system should support disjoint mutable sharing
  for lock-free structures before requiring a mediated or accepted boundary.
- Complete standalone fences, remaining fetch operations, contention tests,
  and the treatment of relaxed visibility in concurrency proofs. The first
  load/store/fetch_add/swap/compare_exchange slice already preserves validated
  orderings into exact x86_64/aarch64 lowering.
- Implement normalized runtime behavior in the existing provider-plan and
  admission spine. Suspension is checked locally against effects; CPU/thread/
  address demands join provider behavior plus preemption granularity.
  Unproved behavior is pessimistic unless an admission receipt authorizes
  reliance on a narrower claim.
- Implement the settled admitted-artifact loader ladder: reusable sealed
  artifact qualification, linear extent-backed placement, freeze, final
  validation, synchronous visibility, and installed-code claim. There is no
  arbitrary byte-to-code conversion or runtime-generated host code.
- Finish constraint-bearing placement and fragmented symbolic materialization;
  implement the settled Cathedral x86 exception/IRQ stack policy and extend the
  root ledger with stack/work/state ceiling-realization-receipt columns.

## Domains, proofs, and arithmetic

- Settle the source spelling for decision 23's one joint ranking across a
  mutually cyclic SCC whose participants expose differently shaped subjects.
  The every-cyclic-edge-decreases semantics and private-witness identity are
  already fixed.
- Decide how much predicate/domain inference to attempt beyond executable
  predicate bodies, explicit evidence, and flow narrowing.
- Finish decision 19's deferred spaces: external quantity-kind equations,
  general open-family linking, introduction/mint-authority grammar,
  `weakens_to` certificate syntax, and affine quantities.
- Decide whether invariant windows may ever carry graph-edge proof debt; the
  current rule treats transitions as consumption points.
- Specify how weakened machine invariants appear in target-state signatures.
- Define checked-result arithmetic, if it earns a distinct library surface
  beyond exact-by-default obligations and explicit policies.
- Define how `Real` contracts are instantiated for `f32`/`f64`: explicit error
  bounds, inferred bounds, or named approximation policies.
- Extend sequence-wide text proofs beyond validate-once plus preservation
  lemmas when richer inductive invariants have a concrete customer.

## Core surface and types

- Settle the authored core-operator declarations for indexing, subslicing,
  arithmetic, and concatenation.
- Finish the boundary between browsable core declarations and
  compiler-managed primitive carriers. Current direction keeps `Array`, `Vec`,
  and `Slice` public; text is bytes plus an encoding domain.
- Finish case payload binding, generic payloads, tag/payload layout, and the
  relation between case-union domains and exhaustiveness.
- Define foreign-type domain imports, orphan/coherence restrictions, and their
  authority-report representation.
- Implement generic bounds for `copy`, `linear`, `zero_init`, `sized`, and the
  parameterized carry policy, then decide whether any further core properties
  earn inclusion.
- Finish conformance-item parsing, both-foreign orphan rules, and
  partially-satisfied diagnostics.

## Boundaries, assembly, and components

- Implement parsed checked assembly, define the first per-target instruction
  catalogs and user/deriver-only availability, and specify final emitted
  machine-state footprint evidence. Opaque/manual raw assembly is not an
  alternative.
- Specify the separately compiled component artifact and ABI, including
  bounded multi-version coexistence, pinned import slots, version budgets,
  outbound calls from old continuations, and eventual continuation migration.

## Tooling and build-time execution

- Decide which contextual words, if any, must become globally reserved. The
  default remains to avoid new reserved words.
- Design an in-language test item/discovery surface; canaries remain external.
- Finish member reflection (`Self::fields`, field/case splices), the full set of
  constant positions, and proof checking of generator-expanded bodies.
