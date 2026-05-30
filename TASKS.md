# Tasks

This is the working backlog, not a history dump. Keep it biased toward what we
should do next.

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

## Resolved Design Decisions (frozen)

Implementation slices below build against these. Minor/easily-reversible details
(exact namespace casing, builtin view surfacing) are left to the owning slice.

1. **Measure declarations (termination).** Custom well-founded orderings use a
   dedicated `measure` keyword as a standalone item:
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
   DescriptorConstruction | Allocation | HostAbiCall`. Core primitives bind a
   named provider; host providers are target-package metadata (generalizing the
   existing `HostAbiPlan`/`HostBoundaryPolicy` whitelist). Only whitelisted
   (core/host/toolchain) packages may declare providers; every boundary binding
   must resolve to a registered provider; unregistered names are rejected. The
   emitted boundary report is the audit artifact.
5. **Text types.** Owned text stays `String` (capacity/`push_str`); the borrowed
   text window is its own type spelled `&string`/`&mut string` (lowercase
   `string`, casing distinguishes owner from window). `StrView`/`&str` naming is
   retired. The window shares the slice `{ptr,len}` descriptor carrier. Expose
   `length`/`non_empty` measures first (cheap, O(1)); `no_nul`/`utf8` are domains
   established at validating boundary constructors and carried as facts, never
   re-proved per use.
6. **Fat descriptor model + owner.** One `FatDescriptor { ptr@0, len@pointer_size
   }` (size `2*pointer_size`, pointer-aligned) covers slices and text windows;
   slice `len` is an element count, text `len` a byte count (kind tag). Owned vs
   borrowed share layout, differing only by an ownership tag in the semantic
   spine. `omega-runtime-abi` owns the shape (field-offset + subslice accessors);
   `omega-layout` and instruction-selection are consumers.

## Next Up (highest leverage)

**KNOWN BUG — x86_64 r12 codegen (unblocks ~68 runtime `*_canary_runs`).** Native
Windows x64 PEs for multi-state dispatch machines crash with `0xC0000005`.
Diagnosed (NOT environmental — `cli_mvp` runs fine): the dispatch-loop /
state-write path loads `r12` with a 32-bit `mov r12d, imm32` (`41 BC`, 4-byte
immediate) at sites where the relocation stage registers a 64-bit `Absolute64`
(8-byte) relocation. The reloc's high 4 bytes spill past the immediate and
execute as `add [rax],eax` → AV. r12 is the only register loaded 32-bit (others
use `movabs reg64,imm64`). Fix: emit `movabs r12, imm64` (`49 BC`, 10 bytes) for
r12 loads carrying an `Absolute64` reloc (keep the 32-bit form for small
non-relocated dispatch indices); reconcile reloc byte_offset/width. Files:
`omega-isa-x86_64/src/lib.rs` `append_mov_r12d_imm32` (~L748),
`encode_dispatch_loop_enter_bytes`/`encode_dispatch_state_write_bytes` (~L74-98);
`omega-relocations/`. Repro: build+run
`canaries/pass/control_flow/runtime_local_scalar_comparison_value_exit` (must
exit 76, currently AVs). Fixing this unblocks all runtime/backend verification.

**`salvage-tm-scc` branch — partial SCC/cycle termination.** Adds
`checks/termination/cycles.rs`, SCC graph reasoning, cyclic state-arg facts, and
mutual-recursion canaries, but does NOT compile as-is: de-dup the duplicated
`GuardedEdge` block in `checks/termination/ranking/patterns.rs`, fix ~4 build
errors, and resolve a `canary_suite.rs` registration conflict, then merge.

**Unfinished parallel lanes (no work landed; clean re-run).** O2 = expression-
level operator `spelling` dispatch + sourcing `items[i]` bounds from the
selected operator's `requires` + domain-operator ambiguity validation. Pr =
consume the proof-lemma/quantified-fact shapes in the checker + boundary proof
obligations + runtime-checkable domain-union validation + the `result`-binder
substitution (so `ensures result in Domain` flows to a call's return value; TODO
left in core `str.omg` `from_utf8`/`from_no_nul`). Ow = ownership events into
slice/string operators + the `Vec`-mutation-while-borrowed rule. See the
[[parallel-agent-orchestration]] memory for the parallel-wave workflow and its
gotchas (verify `cargo build` first).

## Vertical Slices

### Capabilities And Authority

- [ ] Make capability facts flow through returns/derives across nested calls, not
  just direct boundary calls.

### Core Boundary Primitive Registry

- [ ] Populate `BoundaryProvider.contract_ref`/`effect_set`/`target_applicability`
  from the bound operator instead of empty defaults.

### Proof-Backed Indexing And Subslicing

- [ ] Thread refined subslice diagnostics through operator-contract errors once
  `Slice::from/to/range` contracts drive checking directly.
- [ ] Broaden state-argument fact propagation, and extend guard facts, into
  recursive/cyclic state-call argument paths (started on `salvage-tm-scc`).
- [ ] Represent length facts and window-shrinking facts as first-class slice
  proof vocabulary (non-empty already exists).
- [ ] Ensure alias and borrow facts understand subslice overlap conservatively.

### Slice Runtime Descriptor Semantics

- [ ] Generalize subslice descriptor pointer offsets beyond fixed-array alias
  copy special cases (the `FatDescriptorAbi::subslice` seam exists; widen its
  callers past literal fixed-array bases — needs the r12 emission fix to verify).
- [ ] Generalize start-only/end-only/bounded descriptors beyond literal
  fixed-array-backed views.
- [ ] Promote pending subslice canaries into pass/fail suites as descriptor
  lowering becomes real.
- [ ] Keep backend reports explicit about descriptor construction and mutation.

### Measures, Orderings, And Rankings

- [ ] Support builtin/default inference for plain `decreases value` only when
  unambiguous.
- [ ] Replace arithmetic-facing proof UX such as `limit - index` with named
  bounded-distance rankings.
- [ ] Broaden termination checking toward SCC/cycle reasoning (partial on
  `salvage-tm-scc`).
- [ ] Add a runtime exit canary for shrinking-slice recursion once runtime
  dispatch reliably executes descriptor updates (blocked on emission).

### Operators And Domains

- [ ] Use operator symbols during expression-level overload resolution; validate
  ambiguous operator declarations by signature and context (O2 lane).
- [ ] Model `items[index]` and `items[1..]` as core `Slice`/`Array`/`Vec`
  operator contracts whose `requires` drives bounds checking (O2 lane).
- [ ] Define the first semantic domain-operator representation; validate
  ambiguous domain-operator candidates by signature, receiver, and proof context;
  prove only current-context facts can select a domain-operator meaning; add
  pass/ambiguity canaries.

### Ownership, Borrowing, And Views

- [ ] Extend type-aware ownership events into slice/string operators and future
  user-defined copy/drop policy.
- [ ] Teach remaining value-expression analysis to append ownership
  transfer/drop events into checked-flow ownership arenas.
- [ ] Lower abstract ownership summaries into explicit backend transfer and
  cleanup operations.
- [ ] Distinguish more disjoint fixed windows and bounded range/range cases
  where provable.
- [ ] Ensure `Vec` mutation/reallocation rejects while borrowed views exist
  (pending canary `borrow/vec_view_invalidated_by_push`).

### Array, Vec, String, And Views

- [ ] Design `Vec[T]` as owned dynamic storage with length and capacity (surface
  declared; real storage/lowering pending).
- [ ] Back `Array::as_slice`/`as_mut_slice` with real boundary-primitive
  lowering (declared as contracts today).

### Runtime And Backend Confidence

- [ ] Reduce duplicate descriptor assumptions remaining across backend crates.
- [ ] Strengthen assigned-target allocation toward a real register/stack
  allocation story with register classes, spills, and post-assignment cleanup.
- [ ] Reduce host/runtime special-case lowering around stdin/stdout/process
  calls; build richer multi-step text flows and real console interaction.
- [ ] Broaden persistent machine/state mutation coverage beyond isolated
  micro-shapes toward dungeon-sample blockers.
- [ ] Link final-image imports/fixups back to source and lowered boundary-edge
  summaries for reporting and target-policy validation.

## Standing Rules

### Cleanup

- Only split modules when a file owns multiple semantic nouns, blocks a vertical
  slice, or hides a query/canary boundary.
- Keep representation roots explicit when a stage carries both executable shape
  and preserved semantic evidence; keep root constructors and canaries for any
  durable root shape.
- Keep `lib.rs`/`mod.rs` as boundary declarations, not junk drawers.
- Prefer arena/handle/handlespan storage over nested tiny allocations for durable
  IR.

### Canaries

- Three honest categories: `pass` = supported, `fail` = intentionally rejected
  (focused on intended diagnostics), `pending` = desired behavior known but
  implementation behind. Promote pending quickly when fixed; don't let
  compile-only pass canaries imply runtime support.
- Pre-existing red unrelated to current work: `pass_canaries_compile` aborts on
  `calls/mutable_output_host_call` (missing Stdin host binding), and the
  `*_canary_runs` execution tests fault until the r12 emission bug is fixed.

### Docs

- Add a dedicated guide section/chapter for core semantic types once syntax
  stabilizes; add navigable core docs alongside `omega/language/core`.
- Keep traits/modules/host-boundaries sequencing coherent; keep speculative
  topics clearly labeled as working direction.
