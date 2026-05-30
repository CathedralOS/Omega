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

**EMISSION — runtime canary tail (57 pass / 14 fail, filtered `_runs` suite).**
The `0xC0000005` access-violation class is CLOSED: zero-byte instructions (e.g.
`EvaluateDispatchGuard`/`CompareRuntimeText`, whose compare folds into the next
instruction) used to anchor an `Absolute64` data-address relocation at the next
instruction's start and splatter the address into it; a central guard in
`omega-relocations/.../instruction_records/context.rs` (`insert_data_address`
no-ops when `selected_text_width == 0`) now covers every data-address arm. No
remaining failures are relocation bugs. The 14 remaining `_runs` failures bucket
into three causes, none of them relocations:
- **x86 runtime value operand unimplemented (5):** `not implemented yet` compile
  error — `runtime_machine_owned_indexed_integer_write`,
  `runtime_mutable_local_indexed_parameter_write`, `runtime_nested_subslice_dynamic_index`,
  `runtime_slice_index_read`, `runtime_slice_index_read_dispatch`. Implement the
  x86_64 runtime value operand lowering in instruction selection / `omega-isa-x86_64`.
- **Stdin host binding (2):** `missing host binding for runtime text read operation
  Stdin.read` — `runtime_ordered_room_dispatch_loop`, `..._real_show_states` (and
  the same blocks `pass_canaries_compile` on `calls/mutable_output_host_call` and
  the dungeon PE). Wire the Windows x64 `Stdin.read` host binding.
- **Wrong dispatch branch selection (~7):** compiles + runs cleanly, no AV, but
  guarded/ordered state dispatch routes to the WRONG arm — consistently the
  base/false/earlier arm (`runtime_ordered_room_dispatch_{exit,after_call,game_shape,large_machine}`
  70/80/90/100 vs 73/83/93/higher; `runtime_direct_boolean_conjunction` 10 vs 21;
  `runtime_slice_alias_indexed_string_field_concat` 78 vs 77).
  ROOT CAUSE (diagnosed, fix not yet landed): the dispatch **guards are dropped to
  zero width** during emission, so every arm enters unconditionally and the first
  wins. Evidence: `.text` for the ordered-room `route` block emits only
  `mov r12d,<index>; jmp loop` with NO field-comparison `cmp`; pipeline artifacts
  show the scrutinee field refs (`current_room`, `bat_defeated`) present in
  `06_state_graph`/`07_control_flow` but GONE by `08_abstract_operations` (24
  `Guard` ops survive but with no resolved storage operands). The emission filter
  (`omega-machine-emission/src/layout.rs:97` width + `instruction_bytes.rs:166`
  bytes) only emits real `cmp`+`jcc` for `EvaluateDispatchGuard { guard_lowering:
  CompareStaticValue, has_storage: true }`; any other lowering (e.g.
  `NeedsRuntimeExpression`) silently gets width 0 / no bytes. So guards resolve to
  `NeedsRuntimeExpression` because **`resolve_guard_operand_layout` in
  `omega-state-guards/src/operands/layout.rs` returns `None`** for the `self.field`
  scrutinee columns of these ordered multi-column transitions (likely
  `path_targets_source_machine` / `field_layout_by_symbol_or_name` failing to match
  the machine field layout), forcing `guard_lowering()` (builder.rs:101) to
  `NeedsRuntimeExpression`. THE FIX is in `omega-state-guards` operand resolution,
  NOT instruction-selection or relocations. Two follow-ups worth doing alongside:
  (a) make the emission filter a hard error instead of silently dropping a
  non-`CompareStaticValue` guard to zero bytes — that silent drop is what let this
  miscompile instead of failing loudly; (b) instrument `lower_guard_leaf`
  (`omega-state-guards/src/conjunction.rs` ~L234) printing operand kind/storage/
  offset + resulting lowering, run the freshly-built `target/debug/omega-cli.exe`
  directly and capture stderr, to confirm exactly why operand layout is `None`.
Harness: bin `target/debug/omega.exe`; runtime canaries run as
`cargo test -p omega-compiler --test canary_suite _runs -- --test-threads=1`;
regression guard `omega --target windows_x64 samples/cli_mvp/main.omg` (exit 0)
+ `windows_x64_cli_mvp_emits_runnable_pe`.

**EMISSION — unimplemented x86_64 runtime value operand.** A few canaries fail to
*compile* (not crash) with `X86_64 runtime value operand is not implemented yet`
(e.g. `runtime_machine_owned_indexed_integer_write`,
`runtime_mutable_local_indexed_parameter_write`). Implement the missing x86_64
runtime-value operand lowering in instruction selection / `omega-isa-x86_64`.

**EMISSION — Stdin host binding (5 canaries + dungeon PE).** `pass_canaries_compile`
and the stdin/ordered-room canaries abort with `missing host binding for runtime
text read operation Stdin.read`; `windows_x64_dungeon_crawler_emits_runnable_pe`
depends on it. Wire the Windows x64 Stdin.read host binding (this is the
pre-existing red that predates the parallel waves).

Note: `capability_pass_canaries_compile_in_isolation` can show a spurious FAILED
under full-suite parallelism (build-dir race); it passes run alone / with
`--test-threads=1`. Not a real failure.

## Vertical Slices

### Capabilities And Authority

- [ ] Make capability facts flow through returns/derives across nested calls, not
  just direct boundary calls.

### Core Boundary Primitive Registry

- [ ] Populate `BoundaryProvider.contract_ref`/`effect_set`/`target_applicability`
  from the bound operator instead of empty defaults.

### Proof-Backed Indexing And Subslicing

- [ ] Thread refined subslice diagnostics through operator-contract errors once
  `Slice::from/to/range` contracts drive checking directly. (Bounds obligation
  now sources from the spelled operator's `requires` — extend the diagnostics.)
- [ ] Represent length facts and window-shrinking facts as first-class slice
  proof vocabulary (non-empty already exists).
- [ ] Ensure alias and borrow facts understand subslice overlap conservatively.

### Slice Runtime Descriptor Semantics

- [ ] Audit descriptor-backed fixed-index reads/copies now that writes and local
  descriptor materialization work for `rooms[0].exits.as_mut_slice()`. Native
  dungeon initialization no longer crashes at that descriptor shape, but room
  lookup/render still observes blank room data. Recent progress fixed the
  descriptor-header copy bug: `RoomLookup::find_room.apply_room` now emits
  `frame_fixed_indexed(descriptor@..., index 0, elem 232, field +...)` reads and
  fixed-indexed field copies instead of copying the slice descriptor header.
  Remaining bug appears to be string/text descriptor initialization or
  materialization: labels/descriptions/path commands still render blank/NUL even
  after the room struct fields are copied through descriptor element reads.
- [ ] Generalize subslice descriptor pointer offsets beyond fixed-array alias
  copy special cases (the `FatDescriptorAbi::subslice` seam exists; widen its
  callers past literal fixed-array bases — several `runtime_subslice_*` canaries
  still need runtime verification after the zero-byte relocation fix).
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
- [ ] Add a runtime exit canary for shrinking-slice recursion once runtime
  dispatch reliably executes descriptor updates (blocked on emission).

### Operators And Domains

- [ ] Consolidate the two operator-resolution surfaces that landed in parallel on
  two machines. A remote branch added a checked-trees fact layer
  (`omega-checked-trees/src/operators.rs` +
  `omega-typed-trees-to-checked-trees/src/operators.rs` + `checks/operators.rs`:
  operator use facts, spelling candidates, receiver-type narrowing, use origins,
  ambiguity diagnostics, candidate contract spans, contract-bearing uses) while
  the local O2 lane added a typed-trees dispatch API
  (`omega-typed-trees/src/operator.rs::resolve_spelling`), validation ambiguity
  (`omega-validation/src/operators/dispatch.rs`), and the bounds-from-`requires`
  seam. They compile + test together but overlap conceptually — pick one
  authority and route the other through it. Recent progress: checked candidates
  now preserve the exact typed contract span, so proof lowering can inspect the
  selected operator's contracts rather than relying on a count; resolved
  operator contracts now materialize under `ProofFacts.contract_operator_uses`
  with explicit operator contract semantic origins and an acceptance-view surface.
- [ ] Prove that only facts in the CURRENT context can select a domain-operator
  meaning. (Spelling dispatch, bounds-from-`requires`, and competing-meaning
  rejection now exist; the positive proof-context selection is the remaining gap.)

### Ownership, Borrowing, And Views

- [ ] Continue appending ownership transfer/drop events from the remaining
  value-expression sites (operator-result + let-init seams now covered).
- [ ] Lower abstract ownership summaries into explicit backend transfer and
  cleanup operations.
- [ ] Promote `borrow/vec_view_invalidated_by_push` from pending once `Vec[T]`
  lowering exists (the borrow rule fires; blocked only on the `Vec<T>` type being
  usable — exercised today via an array/call-mutation analogue).

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
- Known red (see "Next Up"): `pass_canaries_compile` + stdin canaries abort on
  the missing `Stdin.read` host binding; ~26 `*_canary_runs` still fault on the
  remaining zero-byte-instruction relocation arms; a few fail to compile on the
  unimplemented x86_64 runtime value operand. Runtime canaries currently 46 pass.

### Docs

- Add a dedicated guide section/chapter for core semantic types once syntax
  stabilizes; add navigable core docs alongside `omega/language/core`.
- Keep traits/modules/host-boundaries sequencing coherent; keep speculative
  topics clearly labeled as working direction.
