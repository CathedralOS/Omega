# Tasks

This is the working backlog, not a history dump. Keep it biased toward what we
should do next and only retain recent completed work when it helps restore
context after a break.

Omega's current north star: make core semantic concepts browsable and
proof-backed at the language level, while keeping unsafe/compiler/runtime
representation machinery behind a deliberate boundary.

## Current Strategic Focus

- Drive vertical slices instead of endless cleanup. Refactor when it unblocks a
  feature, clarifies semantic ownership, or adds a canary.
- Make capabilities/authority, proof-backed indexing/subslicing, ranking views,
  and core boundary primitives real end-to-end concepts.
- Keep the compiler pipeline organized around the semantic nouns it owns:
  places, values, facts, loans, moves, drops, calls, transitions, effects, and
  boundary edges.
- Keep `pass`, `fail`, and `pending` canaries honest. Do not let compile-only
  success imply runtime or proof support.

## Recent Completed Context

- Pipeline architecture docs now define semantic ownership and include the
  stage-by-stage ownership matrix.
- Syntax, symbol-resolved, typed, checked, state-graph, control-flow, abstract,
  target, assigned, machine-instruction, machine-program, encoded-machine,
  object, relocation, final-image, and backend-artifact roots now expose
  explicit root constructors in the important places.
- Checked facts now group proof, invariant/domain, value, borrow, flow,
  boundary, ownership, and admissibility evidence behind named roots and query
  surfaces.
- Capability/authority work now has an initial checked-fact root for
  uses/returns/acquires/stores/derives flow records.
- The checked-tree capability manifest now reports effect bits plus
  capability-flow counts, even before population is implemented.
- State-graph and control-flow semantic roots now preserve proof, invariant,
  contract, value, boundary, borrow, and ownership evidence as explicit noun
  roots.
- State-graph-to-control-flow cleanup recently split generic handle/span
  mechanics from noun-specific handle remappers, and unified operation/
  transition conversion policy with code-shape canaries.
- Backend semantic summaries now preserve value, boundary, and ownership
  metadata through abstract, target, assigned, symbolic machine, machine
  program, encoded bytes, and backend artifact roots.
- Core source now has browsable `Slice`, `Array`, `Vec`, `String`, `StrView`,
  `Ptr`, `Slice::Length`, and `Nat::Descending` direction, with core
  implementation edges spelled as `boundary operator`.
- Range/subslice checking has meaningful pass/fail coverage for literals,
  local integer facts, guards, requires clauses, non-empty facts, and refined
  diagnostics.
- Slice runtime descriptors have initial literal fixed-array-backed coverage,
  including composed literal windows and dynamic reads through local aliases.
- Pending canaries exist for known future behavior, including custom ranking
  projection through struct views.

## Vertical Slices

### Capabilities And Authority

Goal: make authority flow visible at package, language, and host-boundary
levels without drowning the language in keywords.

- [ ] Populate checked capability facts from typed calls, boundary signatures,
  package declarations, and host-boundary edges.
- [ ] Extend the initial entry capability manifest into a package/report
  surface for theoretical blast radius: what a library can use, acquire,
  return, store, or derive.
- [ ] Connect boundary/host calls to capability facts so target policy checks
  can say whether a host call is allowed for the package.
- [ ] Add canaries for:
  - library uses caller-provided folder capability
  - library acquires filesystem authority
  - library stores a capability
  - unapproved host call is rejected or reported

### Core Boundary Primitive Registry

Goal: stop treating `boundary operator` names as vibes. Core/compiler/runtime
providers should be auditable.

- [ ] Define the language-authored registry shape for compiler/runtime
  primitive providers such as slice indexing, pointer offset, descriptor
  construction, allocation, and host ABI calls.
- [ ] Decide whether the registry is package/target metadata, restricted core
  declarations, emitted compiler inventory, or a combination.
- [ ] Require boundary implementation bindings to reference registered
  providers once binding syntax exists.
- [ ] Reject unregistered boundary provider names outside explicitly
  whitelisted toolchain/core packages.
- [ ] Add canaries for accepted core primitive bindings and rejected
  unregistered bindings.

### Proof-Backed Indexing And Subslicing

Goal: model indexing and slicing as proof-backed core operators, not parser
special cases with bolted-on checks.

- [ ] Thread refined subslice diagnostics through operator-contract errors once
  `Slice::from/to/range` contracts drive checking directly.
- [ ] Connect range validity facts to indexing validity facts instead of
  duplicating proof logic.
- [ ] Broaden state-argument fact propagation to recursive/cyclic control-flow
  paths instead of direct-call/transition seeds only.
- [ ] Extend guard facts into recursive and cyclic state-call argument
  propagation.
- [ ] Decide how inclusive/exclusive range forms spell and lower.
- [ ] Represent non-empty facts, length facts, and window-shrinking facts as
  first-class slice proof vocabulary.
- [ ] Ensure alias and borrow facts understand subslice overlap
  conservatively.

### Slice Runtime Descriptor Semantics

Goal: make runtime slice/string descriptors line up with the proof model.

- [ ] Generalize subslice descriptor pointer offsets beyond fixed-array alias
  copy special cases.
- [ ] Generalize start-only/end-only/bounded descriptors beyond literal
  fixed-array-backed views.
- [ ] Choose one clear backend representation owner for descriptor writes,
  reads, pointer offsets, and lengths.
- [ ] Promote pending subslice canaries into pass/fail suites as descriptor
  lowering becomes real.
- [ ] Keep backend reports explicit about descriptor construction and mutation.

### Measures, Orderings, And Rankings

Goal: support named well-founded views without saying "cards have one global
order."

- [ ] Replace temporary operator-like ranking declarations with dedicated
  ranking or measure syntax once selected.
- [ ] Decide how order/measure declarations represent "rank this value by this
  view."
- [ ] Support builtin/default inference for plain `decreases value` only when
  unambiguous.
- [ ] Replace arithmetic-facing proof UX such as `limit - index` with named
  bounded-distance rankings.
- [ ] Add lexicographic ranking support.
- [ ] Support multiple named orders for the same data shape.
- [ ] Extend custom ranking projections from explicit field expressions to
  full user-defined struct views such as `decreases card -> Card::PowerOrder`.
- [ ] Broaden termination checking beyond narrow direct self-recursion toward
  SCC/cycle reasoning.
- [ ] Add a runtime exit canary for shrinking-slice recursion once runtime
  dispatch reliably executes descriptor updates instead of hanging.

### Operators And Domains

Goal: operators should have visible semantic homes and domain-selected meanings
without hidden runtime tags.

- [ ] Use operator symbols during overload resolution and validate ambiguous
  operator declarations by signature and context.
- [ ] Design a declaration form for fixed spellings such as `+`, `[]`, and
  range slicing.
- [ ] Model `items[index]` and `items[1..]` as core `Slice`/`Array`/`Vec`
  operator contracts.
- [ ] Design boundary implementation bindings for core operators without
  hiding signatures or proof obligations.
- [ ] Define the first semantic domain-operator representation.
- [ ] Validate ambiguous domain operator candidates by signature, receiver, and
  proof context.
- [ ] Prove that only facts in the current context can select domain operator
  meanings.
- [ ] Add canaries for successful domain-selected operators and ambiguity
  errors.

### Ownership, Borrowing, And Views

Goal: make ownership facts precise enough that views, arrays, strings, and
future vectors do not rely on happy-path alias assumptions.

- [ ] Make ownership event production fully type-aware so copy/no-drop values
  and ownership-consuming values are distinguished across all transfer sites.
- [ ] Extend type-aware ownership events into slice/string operators and future
  user-defined copy/drop policy.
- [ ] Teach remaining value-expression analysis to append ownership
  transfer/drop events into checked-flow ownership arenas.
- [ ] Lower abstract ownership summaries into explicit backend transfer and
  cleanup operations.
- [ ] Repair direct owner assignment rejection while a local borrow alias
  remains active.
- [ ] Distinguish more disjoint fixed windows and bounded range/range cases
  where provable.
- [ ] Ensure `Vec` mutation/reallocation rejects while borrowed views exist.
- [ ] Add array, slice, string, and future vec aliasing canaries.

### Array, Vec, String, And Views

Goal: owners and borrowed views should share one proof/runtime story.

- [ ] Make fixed arrays visible as `Array[T; N]` or an equivalent core
  concept.
- [ ] Define `Array::as_slice` and `Array::as_mut_slice` as visible
  operator/machine contracts backed by boundary primitive lowering where
  needed.
- [ ] Design `Vec[T]` as owned dynamic storage with length and capacity.
- [ ] Define how `Vec` borrowing prevents reallocation or mutation that would
  invalidate active slices.
- [ ] Decide whether current `String` remains the public owned text name or
  evolves toward `Str`.
- [ ] Define `StrView` or equivalent borrowed text view semantics.
- [ ] Decide whether string views are byte slices with text domains or their
  own core view type.
- [ ] Expose string/text measures and domains such as length, non-empty, UTF-8,
  and no-NUL from a browsable core surface.

### Runtime And Backend Confidence

Goal: reduce special-case bring-up behavior and make native output less
fictional.

- [ ] Identify one representation model for fat descriptors and pointer-based
  carriers.
- [ ] Reduce duplicate descriptor assumptions across backend crates.
- [ ] Strengthen assigned-target allocation toward a real register/stack
  allocation story with register classes, spills, and post-assignment cleanup.
- [ ] Reduce host/runtime special-case lowering around stdin/stdout/process
  calls.
- [ ] Build multi-step text flows with richer transitions.
- [ ] Increase host/runtime confidence around real console interaction paths.
- [ ] Broaden persistent machine/state mutation coverage beyond isolated
  micro-shapes toward dungeon-sample blockers.
- [ ] Link final-image imports/fixups back to source and lowered
  boundary-edge summaries for reporting and target-policy validation.

### Proof And Domains

Goal: move beyond local first-order facts without pretending we have Lean.

- [ ] Add reusable proof lemmas for length, bounds, and window transformations.
- [ ] Add quantified or sequence-style facts for text/slice invariants.
- [ ] Repair dynamic indexed domain-fact preservation across disjoint mutating
  calls; current unit coverage still rejects some `self.index` proofs.
- [ ] Improve diagnostics when a proof-backed operator is missing a required
  fact.
- [ ] Define boundary proof obligations for host/core primitive
  implementations.
- [ ] Keep domain unions/intersections executable only when their bodies are
  runtime-checkable.

## Cleanup Rules

- [ ] Only split modules when a file owns multiple semantic nouns, blocks a
  vertical slice, or hides a query/canary boundary.
- [ ] Keep representation roots explicit when a stage carries both executable
  shape and preserved semantic evidence.
- [ ] Keep root constructors and canaries for any durable root shape.
- [ ] Keep `lib.rs` and `mod.rs` as boundary declarations, not implementation
  junk drawers.
- [ ] Prefer arena/handle/handlespan storage over nested tiny allocations for
  durable IR.

## Canary Discipline

- [ ] Maintain three honest categories:
  `pass` means supported, `fail` means intentionally rejected, and `pending`
  means desired behavior is known but implementation is behind.
- [ ] Promote pending canaries quickly when fixed.
- [ ] Add pending canaries for serious known gaps instead of leaving them as ad
  hoc probes.
- [ ] Keep fail canaries focused on intended diagnostics.
- [ ] Avoid letting compile-only pass canaries imply runtime support.

Known pending canary:

- `canaries/pending/termination/custom_ranking_struct_view_unimplemented`
  should become a pass when ranking views can project through declared bodies.

## Docs

- [ ] Add a dedicated guide section/chapter for core semantic types once syntax
  stabilizes.
- [ ] Keep traits/modules/host-boundaries sequencing coherent.
- [ ] Add navigable core docs alongside `omega/language/core` once declarations
  exist.
- [ ] Keep speculative topics clearly labeled as working direction.
