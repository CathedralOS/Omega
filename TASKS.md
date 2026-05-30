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

## Resolved Design Decisions (Wave 0)

These forks are now decided and frozen. Implementation slices below build against
them. Minor/easily-reversible details (exact namespace casing, builtin view
surfacing) are left to the owning slice.

1. **Measure declarations (termination).** Custom well-founded orderings use a
   dedicated `measure` keyword as a standalone item, replacing the temporary
   `operator Type::Order(...)` hack the checker sniffs by shape today:
   `measure Card::PowerOrder(card: Card) -> usize { card.power }` and
   `measure Quest::Difficulty lexicographic { tier, remaining_steps }`. Use site
   `decreases value -> Type::Name` is unchanged. Multiple measures per type and
   lexicographic tuples are supported.
2. **Range forms.** `a..b` exclusive, `a..=b` inclusive (plus open `a..`, `..b`,
   `..`). Inclusive normalizes to `a..(b+1)`. Exclusive end requires `b <= len`
   (range-bound facts); inclusive end requires `b < len` (index facts) — this is
   how range validity connects to index validity; inclusive non-empty ranges
   also establish a `non_empty` fact. The `..=MAX` overflow edge is a proof
   error (`checked_add`), not a panic.
3. **Operator spellings.** Fixed spellings are declared with an optional
   `spelling` clause on a named `operator`
   (`... -> T spelling [] requires index < items.len;`). Overload key stays path
   + parameter types. `items[index]`/`items[1..]` resolve to the spelled core
   operator and its `requires` IS the bounds obligation. The spelling sits above
   the `boundary` modifier, so it never hides signature or proof obligations.
4. **Boundary primitive registry.** One `BoundaryProvider { name, category,
   contract_ref, effect_set, target_applicability, origin_package }` record.
   Categories: `SliceIndexing | PointerOffset | PointerAccess |
   DescriptorConstruction | Allocation | HostAbiCall`. Core primitives authored
   as restricted core declarations binding a named provider; host providers as
   target-package metadata (generalizing the existing
   `HostAbiPlan`/`HostBoundaryPolicy` whitelist). Only whitelisted
   (core/host/toolchain) packages may declare providers; every boundary binding
   must resolve to a registered provider; unregistered names are rejected. The
   emitted boundary report is the audit artifact.
5. **Text types.** Owned text stays `String` (capacity/`push_str`); the borrowed
   text window is its own type spelled `&string`/`&mut string` (lowercase
   `string`, casing distinguishes owner from window). `StrView`/`&str` naming is
   retired. The window shares the slice `{ptr,len}` descriptor carrier. Expose
   `length`/`non_empty` measures first (cheap, O(1)); `no_nul`/`utf8` are
   domains established at validating boundary constructors and carried as facts,
   never re-proved per use.
6. **Fat descriptor model + owner.** One `FatDescriptor { ptr@0, len@pointer_size
   }` (size `2*pointer_size`, pointer-aligned) covers slices and text windows;
   slice `len` is an element count, text `len` a byte count (kind tag). Owned vs
   borrowed share layout, differing only by an ownership tag in the semantic
   spine. `omega-runtime-abi` owns the shape (field-offset + subslice
   accessors); `omega-layout` and instruction-selection are consumers and stop
   re-deriving the layout. Migration is byte-identical refactor steps that keep
   the working macOS ARM64 / Windows x64 PE paths green.

## Next Session / Resume Notes

**HIGH-VALUE KNOWN BUG — x86_64 r12 codegen (unblocks ~68 runtime canaries).**
Native Windows x64 PEs for multi-state dispatch machines crash with `0xC0000005`
(access violation). Diagnosed root cause (high confidence, NOT environmental —
`cli_mvp` runs fine): the dispatch-loop / state-write path loads `r12` with a
32-bit `mov r12d, imm32` (opcode `41 BC`, 4-byte immediate) at sites where the
relocation stage registers a 64-bit `Absolute64` (8-byte) relocation. The reloc
writes 8 bytes into the 4-byte immediate field; the high 4 bytes (`01 00 00 00`
of image base `0x140000000`) spill past the instruction and execute as
`add [rax],eax` → AV. Every other register load already uses `movabs reg64,imm64`
(REX.W: r15=`49 BF`, r10=`49 BA`, r11=`49 BB`, rax=`48 B8`); r12 is the only
32-bit one. Fix: emit `movabs r12, imm64` (REX.W.B = `49 BC`, 10 bytes) for r12
loads carrying an `Absolute64` relocation (keep the 32-bit form for small
non-relocated dispatch indices). Files: `omega-isa-x86_64/src/lib.rs`
`append_mov_r12d_imm32` (~L748) + `encode_dispatch_loop_enter_bytes` /
`encode_dispatch_state_write_bytes` (~L74-98); reconcile reloc byte_offset/width
in `omega-relocations/`. Repro: build+run
`canaries/pass/control_flow/runtime_local_scalar_comparison_value_exit` (must
exit 76, currently AVs). Fixing this turns the `*_canary_runs` suite green and
unblocks all runtime/backend verification.

**Wave 3 remaining lanes (not yet started — stopped before first commit):**
O2 (expression-level operator `spelling` dispatch + bounds-from-`requires` +
domain-operator ambiguity), Pr (consume the proof-lemma/quantified-fact shapes;
boundary proof obligations; runtime-checkable domain unions; the `result`-binder
substitution so `ensures result in Domain` flows to a call's return value — TODO
left in core str.omg `from_utf8`/`from_no_nul`), Tm (termination SCC/cycle
reasoning + recursive/cyclic state-arg & guard propagation — **PARTIAL WORK on
branch `salvage-tm-scc`**: adds SCC graph reasoning + cyclic state-arg facts +
mutual-recursion canaries, but does NOT compile as-is — de-dup the duplicated
`GuardedEdge` block in `checks/termination/ranking/patterns.rs`, fix ~4 build
errors, and resolve a `canary_suite.rs` registration conflict before merging),
Ow (ownership events
into slice/string operators; sharper borrow overlap; `Vec` mutation-while-borrowed
rule + promote `canaries/pending/borrow/vec_view_invalidated_by_push`). Tx (text
domains) already landed on main. Lane briefs preserved in session history; see
[[parallel-agent-orchestration]] memory for the parallel-wave workflow.

## Recent Completed Context

- Wave 1 parallel implementation landed six lanes against the frozen Wave 0
  decisions: `measure` keyword + lexicographic/struct-view termination (M),
  `a..=b` inclusive ranges with inclusive-end→index-fact wiring (R),
  `String`/`string` text rename + `String::NonEmpty` + Array/Vec surface (T),
  the single `FatDescriptorAbi` descriptor owner collapsing duplicated
  `{ptr,len}` sites (D), borrow-overlap precision + registered view-aliasing
  canaries (B), and proof-lemma/quantified-fact/boundary-obligation modules (P).
  Whole workspace builds; fail/pending canary suites pass; native PE execution
  canaries remain pre-existing-red in the sandbox (asm-emission gap, not a Wave 1
  regression). Follow-on lanes also landed: operator `spelling` + `BoundaryProvider`
  registry with whitelist/resolution gates (O), dynamic-indexed domain-fact
  preservation coverage + soundness docs (Pd), and capability verb population +
  blast-radius manifest + host-call authorization (Cap). Remaining: expression-level
  spelling dispatch wiring, lemma consumption, runtime descriptor generalization,
  and native/asm-emission depth.
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
- Boundary-trait calls now populate initial `uses` capability-flow facts from
  checked-flow boundary edge evidence.
- Direct calls through boundary-trait signatures now also populate `uses`
  capability-flow facts, with a canary asserting the emitted manifest count.
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

- [x] Broaden checked capability fact population beyond boundary-trait `uses`
  into returns, acquires, stores, derives, package declarations, and host calls.
  (done: all five verbs derived from boundary-call sites off the effect plan.)
- [x] Extend the initial entry capability manifest into a package/report
  surface for theoretical blast radius: what a library can use, acquire,
  return, store, or derive. (done: `CapabilityBlastRadius` rows in the boundary
  report / `10_boundary.html`.)
- [x] Connect boundary/host calls to capability facts so target policy checks
  can say whether a host call is allowed for the package. (done: host-authority
  provider registry + `authorize_host_call` + `unapproved host call` check.)
- [x] Add canaries for:
  - library uses caller-provided folder capability
  - library acquires filesystem authority
  - library stores a capability
  - unapproved host call is rejected or reported

### Core Boundary Primitive Registry

Goal: stop treating `boundary operator` names as vibes. Core/compiler/runtime
providers should be auditable.

- [x] Define the language-authored registry shape for compiler/runtime
  primitive providers such as slice indexing, pointer offset, descriptor
  construction, allocation, and host ABI calls.
  (done: `BoundaryProvider { name, category, contract_ref, effect_set,
  target_applicability, origin_package }` + `provider <Name> : <Category>;` item.)
- [x] Decide whether the registry is package/target metadata, restricted core
  declarations, emitted compiler inventory, or a combination.
  (decided: Wave 0 #4 — combination: `BoundaryProvider` record, core decls +
  target metadata, emitted report as audit.)
- [x] Require boundary implementation bindings to reference registered
  providers once binding syntax exists.
  (done: `provider <name>` clause on boundary operators + resolution gate.)
- [x] Reject unregistered boundary provider names outside explicitly
  whitelisted toolchain/core packages. (done: whitelist gate on declaring
  package + resolution gate on every binding.)
- [x] Add canaries for accepted core primitive bindings and rejected
  unregistered bindings. (done: operators/accepted_core_provider_binding +
  unregistered_provider_binding + app_package_provider_rejected.)

### Proof-Backed Indexing And Subslicing

Goal: model indexing and slicing as proof-backed core operators, not parser
special cases with bolted-on checks.

- [ ] Thread refined subslice diagnostics through operator-contract errors once
  `Slice::from/to/range` contracts drive checking directly.
- [x] Connect range validity facts to indexing validity facts instead of
  duplicating proof logic. (done: inclusive-end `a..=b` validity reuses the
  strict index vocabulary `b < len`; exclusive stays on range-bound facts.)
- [ ] Broaden state-argument fact propagation to recursive/cyclic control-flow
  paths instead of direct-call/transition seeds only.
- [ ] Extend guard facts into recursive and cyclic state-call argument
  propagation.
- [x] Decide how inclusive/exclusive range forms spell and lower.
  (decided: Wave 0 #2 — `a..b`/`a..=b`, normalize inclusive to `a..(b+1)`.)
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
- [x] Choose one clear backend representation owner for descriptor writes,
  reads, pointer offsets, and lengths. (decided: Wave 0 #6 —
  `omega-runtime-abi` owns the `FatDescriptor` shape; layout/instruction
  selection are consumers.)
- [ ] Promote pending subslice canaries into pass/fail suites as descriptor
  lowering becomes real.
- [ ] Keep backend reports explicit about descriptor construction and mutation.

### Measures, Orderings, And Rankings

Goal: support named well-founded views without saying "cards have one global
order."

- [x] Replace temporary operator-like ranking declarations with dedicated
  ranking or measure syntax once selected.
  (done: `measure` keyword implemented; termination checker now looks up
  declared measures instead of sniffing operator shape.)
- [x] Decide how order/measure declarations represent "rank this value by this
  view." (decided: Wave 0 #1 — standalone `measure Type::Name(...) -> usize { .. }`.)
- [ ] Support builtin/default inference for plain `decreases value` only when
  unambiguous.
- [ ] Replace arithmetic-facing proof UX such as `limit - index` with named
  bounded-distance rankings.
- [x] Add lexicographic ranking support.
  (done: `measure Type::Name lexicographic { a, b }` + lexicographic ranking arm.)
- [x] Support multiple named orders for the same data shape.
  (done: multiple `measure` declarations per type.)
- [x] Extend custom ranking projections from explicit field expressions to
  full user-defined struct views such as `decreases card -> Card::PowerOrder`.
  (done: struct-view ranking arm; pending canary promoted to pass.)
- [ ] Broaden termination checking beyond narrow direct self-recursion toward
  SCC/cycle reasoning.
- [ ] Add a runtime exit canary for shrinking-slice recursion once runtime
  dispatch reliably executes descriptor updates instead of hanging.

### Operators And Domains

Goal: operators should have visible semantic homes and domain-selected meanings
without hidden runtime tags.

- [ ] Use operator symbols during overload resolution and validate ambiguous
  operator declarations by signature and context.
- [x] Design a declaration form for fixed spellings such as `+`, `[]`, and
  range slicing. (decided: Wave 0 #3 — `spelling` clause on named `operator`.)
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
- [x] Decide whether current `String` remains the public owned text name or
  evolves toward `Str`. (decided: Wave 0 #5 — keep `String`.)
- [x] Define `StrView` or equivalent borrowed text view semantics.
  (decided: Wave 0 #5 — borrowed window type spelled `&string`/`&mut string`;
  `StrView` retired.)
- [x] Decide whether string views are byte slices with text domains or their
  own core view type. (decided: Wave 0 #5 — own view type sharing the slice
  `{ptr,len}` descriptor carrier.)
- [ ] Expose string/text measures and domains such as length, non-empty, UTF-8,
  and no-NUL from a browsable core surface.

### Runtime And Backend Confidence

Goal: reduce special-case bring-up behavior and make native output less
fictional.

- [x] Identify one representation model for fat descriptors and pointer-based
  carriers. (decided: Wave 0 #6 — `FatDescriptor { ptr@0, len@pointer_size }`
  owned by `omega-runtime-abi`.)
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
- [x] Repair dynamic indexed domain-fact preservation across disjoint mutating
  calls; current unit coverage still rejects some `self.index` proofs.
  Verified the joined-segment matcher already preserves dynamic-indexed domain
  facts across disjoint field mutations and across disjoint literal-index element
  mutations, while soundly invalidating same-element/unknown-index mutations.
  Added unit coverage in `flow/domain/invalidation/matching/tests.rs`
  (`indexed_domain_dependency_preserves_shared_dynamic_index_disjoint_field`,
  `indexed_domain_dependency_invalidates_distinct_dynamic_index_same_field`),
  pass canaries `domains/call_requires_domain_membership_preserved_across_disjoint_{dynamic_field,literal_element}_mutation`,
  and soundness fail canary
  `domains/call_requires_domain_membership_invalidated_by_same_literal_element_call`;
  documented the conservative index-overlap policy in `flow/place/comparison.rs`.
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

- (cleared) `custom_ranking_struct_view_unimplemented` was promoted to
  `canaries/pass/termination/custom_ranking_struct_view` once the `measure`
  keyword let ranking views project through declared bodies.

## Docs

- [ ] Add a dedicated guide section/chapter for core semantic types once syntax
  stabilizes.
- [ ] Keep traits/modules/host-boundaries sequencing coherent.
- [ ] Add navigable core docs alongside `omega/language/core` once declarations
  exist.
- [ ] Keep speculative topics clearly labeled as working direction.
