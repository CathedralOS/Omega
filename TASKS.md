# Tasks

This is the working backlog, not a history dump. Keep it biased toward what we
should do next.

Omega's current north star: make core semantic concepts browsable and
proof-backed at the language level, while keeping unsafe/compiler/runtime
representation machinery behind a deliberate boundary.

## Current Strategic Focus

- Omega's first real consumer is the Cathedral OS (`../Cathedral`). The gap
  analysis between Cathedral's architectural bets and the language's current
  state lives in [wiki/cathedral_alignment.md](wiki/cathedral_alignment.md) —
  Tier 1 items there (ZII guarantee, wire data semantics, versioned data,
  separate-compilation awareness, concurrency/atomics decisions, freestanding
  target, enum payloads) should bias which vertical slices get picked next.
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
7. **Case members, not `enum`.** Alternatives are a member class of `data`:
   `case` members with named payload fields, shape derived from members
   (record / sum / MIXED; sum-only ships first, mixed is severable). First
   case is the zero case (ZII); no niche layout. A case implicitly declares
   the same-named DOMAIN (free tag-compare classifier), so `case` never
   appears at use sites: match arms are classifications -- case arms and
   domain arms mix with identical `Type::Name` spelling, first satisfied arm
   wins, payload binding only on case arms, exhaustiveness counts only
   decidable arms (cases + case-union domains). Case subsets are domain
   unions (`when self in A | B`), replacing shadow enums.
   Cases/domains/machines share the `Type::member` namespace; collisions are
   hard errors, never priority. Foreign-type domains are allowed
   (extension-trait analog), import-gated, same loud-collision rule. The
   `enum` keyword is retired once `case` parsing lands (today it remains the
   transitional spelling for payload-less sums). See chapters 1 + 8 +
   appendix.

## Next Up (highest leverage)

**Recent canary promotions.** Numeric literal suffixes (`3i32`, `3.0real`,
`3nat`), newline-separated proof facts, field `+=` assignment, relax scope syntax
(`relax target { ... }`), relaxed borrow parameter spelling (`&mut relaxed T`),
trait `default machine` syntax, `data FixedBuffer<T, const N: usize>` const
parameters usable as symbolic fixed-array lengths, and top-level
`host <target> provides <Trait> { machine -> syscall N; }` provider metadata,
plus `wire data` schemas with encoding, numbered fields, reserved tags, and
version blocks, plus `data` historical `version` blocks, plus `&mut dyn Trait`
parameters and trait-method calls on dyn receivers now compile in the active
pass suite. Trailing machine version selectors like `Counter::increment::v1`
now split structurally as an attached-data method instead of treating `v1` as
the entry state. Single-subject transition match arms can now parse data
destructure guards such as `Player { health, .. } if health > 5` by rewriting
the destructured guard name to the matched subject field. Vec slice-view
invalidation now rejects through source-visible `Vec<T>::push`, and the last
physical pending canaries were promoted to active fail coverage for expression
`match` and version migration matching. Full canary suite is green locally at
106 Rust tests passing; pass/fail canary counts can change without changing the
Rust harness test count because many canaries are batched. The proofs false
twins were promoted to `canaries/fail/proofs/` when the contract entailment
engine landed (empty-body proof machines now PROVE or REJECT in-language
contracts); see `wiki/proof_engine_roadmap.md`.

**Inline asm control-flow follow-up.** Current inline asm support is deliberately
narrow: `asm { jmp state(...) }` parses and lowers to an ordinary Omega
transition target. Arbitrary labels/back-edges are actively rejected by fail
canary, while structured load/store mnemonics, register constraints,
clobber/effect declarations, and `asm where` contracts remain unsupported and
should not be faked as generic statements.

**Transition data-pattern follow-up.** Current data-pattern support is a narrow
transition-guard lowering path: `Type { field, .. } if guard` rewrites bare
captured field names inside `guard` to member reads on the single match subject.
Need real pattern binding semantics, multi-field/multi-subject validation,
domain-pattern lowering that proves membership rather than just compiling the
surface, and clearer diagnostics for unsupported destructuring forms.

**Const data parameter follow-up.** Current `const` data parameter support is a
structural compile path: syntax/resolved/typed trees preserve const parameters,
and `[T; N]` carries a symbolic length instead of collapsing to a fake literal.
Uninstantiated symbolic lengths deliberately do not produce concrete layout or
runtime-storage descriptors yet. Need instantiation-time substitution,
duplicate/value-kind validation, layout diagnostics for unresolved symbolic
lengths in non-generic contexts, and operator/range proof integration for
const-length facts.

**Data version semantics follow-up.** Current `data version` support is syntax
metadata only: historical member blocks are parsed/snapshotted and preserved in
syntax trees, while symbol-resolved lowering skips them. Version-scoped machine
paths are recognized structurally (`Counter::increment::v1` attaches `self` to
`Counter` and keeps `increment` as the entry), but do not yet bind to historical
member shapes. Need validation, historical-shape symbols, migration matching,
true version-scoped machine binding, and layout/serialization rules before data
versions are operational.

**Wire data semantics follow-up.** Stage 1 (validation + compatibility) is
done: wire schemas now lower through symbol-resolved and typed trees as their
own root family (`WireSchema` with arena-stored members and a `WireSchema`
symbol kind), `omega-validation` rejects duplicate/reserved tag misuse,
duplicate versions, unresolved field types, and version-vs-current type
changes or unreserved retirements (fail canaries under `canaries/fail/wire/`),
and every compile emits a `04_wire_protocols.txt` compatibility report with
per-version verdicts. Still needed: encoder/decoder generation, runtime layout
of wire values, encoding-family semantics, and version negotiation before wire
schemas are operational.

**Host-provider semantics follow-up.** Current host-provider support is
syntax-preserving metadata: it parses and snapshots syscall mapping rows, but
semantic lowering still ignores the item. Boundary-provider registry validation,
target-package whitelisting, syscall/import lowering, and boundary report
integration still need the real implementation.

**Trait default semantics follow-up.** Current `default machine` support is
structural: the marker flows through syntax/resolved/typed signatures and the
default body is parsed. Trait conformance, implementation reuse, override rules,
and dispatch behavior still need a real semantic pass before default methods are
more than surface syntax.

**Dynamic trait follow-up.** Current `dyn Trait` support is structural and
compile-path oriented: syntax/resolved/typed/checked trees preserve dynamic trait
types, receiver lookup can target trait machines, and layout/runtime-storage use
an explicit dynamic-trait fat descriptor. Need true trait-object construction,
vtable/interface table emission, dynamic dispatch lowering, and validation that
only trait object-safe machines are callable through `dyn Trait`.

**Relax semantics follow-up.** Current relax support is intentionally structural:
syntax is preserved, relaxed reference metadata flows through typed trees, and
relax scopes flatten during syntax-to-resolved lowering after resolving the target.
The invariant-weakening semantics still need a checked-tree/proof pass that marks
which place is relaxed, verifies exclusivity, and restores obligations at scope
exit.

**Runtime frame aliasing bug found while bringing up dungeon.** A slice
descriptor parameter can overwrite the inline array storage it points into when
state parameter slots are reused across a call/transition boundary. Repro shape:
`let rooms = level.rooms.as_slice(); find_room_at(rooms, ...)` where `level` is a
by-value runtime-frame parameter and the callee's `rooms` descriptor slot is
assigned over `level.rooms`. The visible copy range is only the 16-byte
descriptor, so current scratch staging misses the hidden pointee range. Fix in
frame-slot assignment/staging so descriptor payload source ranges are first-class.

**Dungeon sample is functional by bypassing unstable room lookup.** Native dungeon
now drives movement, treasure, combat, fountain, side-room gold, inventory, repeat
use/fight, invalid input, and exits without crashing. This is a sample hardening
fix, not the deeper compiler fix: `RoomLookup::find_room_mut`/dynamic slice search
still misroutes mutable room references, so keep a dedicated canary/backlog item
for mutable slice-returned room lookup before moving generated-room events back
onto `Room` data.

**RESUME POINT (next session).** main `d20d1c06`, synced, clean. Wave A landed
(Cv/Tm/Cap/R/B). Wave B-1: E3 Stdin host binding landed; DB dispatch-branch
diagnosed-not-fixed (see below). The two highest-value backend fixes, both with a
confirmed root cause, are ready to drive (with a human in the loop — autonomous
solo agents reliably DIAGNOSE these but keep failing to LAND them):

!!! INTEGRITY NOTE: a "DIAGNOSIS v4" claiming the bug was `collection.rs` using
`guard_for_source` with a loop-invariant entry key was FALSE (the instrumentation
behind it silently failed to compile, so the claim was never observed) and has been
REVERTED (commit d366b8e2). collection.rs actually uses
`dispatch_guard_comparison(context, state.dispatch_index, order)` →
`guard_for_dispatch(dispatch_index, order)`, which is correctly keyed. Disregard v4.
Trust only the v3 instrumented producer-side data below; the consumer-side splitter
behavior remains UNVERIFIED this session.

VERIFIED CODE-READ FACTS (this session, from reading the files — not instrumented):
- `collection.rs::runtime_dispatch_loop_edges` builds each `RuntimeDispatchLoopEdge`
  from `dispatch_guard_comparison(ctx, state.dispatch_index, order)`
  (`omega-runtime-dispatch-loop/src/loop_plan/guards.rs`), which returns the SINGLE
  stored `guard.lowering`/operands from `guard_for_dispatch`. For a multi-column tuple
  arm that stored lowering is `NeedsRuntimeExpression` (per v3). So the edge carries
  ONE useless guard — there is NO per-column `And` decomposition at the dispatch-loop
  layer; `guard_disjuncts` there only splits `Or`, not `And`.
- The `And`-splitting DOES exist, but only in the OTHER consumer,
  `omega-instruction-selection/.../runtime_dispatch/edges.rs`
  (`lower_guard_conjunction` at :128, conjunct selector at :175), gated on
  `!guard_can_emit_directly(edge)` (returns false for NeedsRuntimeExpression).
UNVERIFIED / NEXT STEP: confirm whether edges.rs's `lower_guard_conjunction` actually
runs and returns clauses for these edges. CRITICAL HARNESS LESSON: this session every
`eprintln!` probe SILENTLY failed to take effect because the Edit didn't match real
code AND/OR the crate wasn't rebuilt — the canary kept exiting 10 from the UNMODIFIED
compiler, which looked like "instrument shows nothing." Before trusting ANY probe:
(1) put the eprintln in, (2) `cargo build -p omega-cli` and CONFIRM the owning crate
line appears in the compile output, (3) `grep -c GUARDDBG target/debug/omega.exe` to
confirm the string is in the binary, THEN run. The genuine v3 build_state_guard probe
DID work (it printed) — model new probes on that, in omega-state-guards/builder.rs.

=== DIAGNOSIS v3 — ANSWERED (instrumented build_state_guard this session) ===
The tuple arms DO produce guard expressions, but they lower to a binary `And`.
Per-arm instrumented output on the 7-arm `route` transition:
  arm0 `(0,_,_,_)`    -> RuntimeEquality Equal  -> CompareStaticValue (works)
  arm1 `(1,true,_,_)` -> RuntimeExpression And   -> NeedsRuntimeExpression
  arms2-5 (multi-col) -> RuntimeExpression And   -> NeedsRuntimeExpression
  arm6 `_`            -> Always -> NoOp
`guard_lowering()` (builder.rs:101) has no `And` arm → `NeedsRuntimeExpression`;
emission drops that to ZERO width → multi-column arms enter unconditionally → first
arm (gate/exit 10) wins instead of ambush/21. BUT the consumer
`select_dispatch_guard_instructions` (omega-instruction-selection/.../runtime_dispatch/
edges.rs) ALREADY decomposes And: behind `if !guard_can_emit_directly(edge)` it calls
`lower_guard_conjunction` (edges.rs:128) + the conjunct selector (:175), and
`guard_can_emit_directly` (edges.rs:279) returns FALSE for NeedsRuntimeExpression
(line 309) so they should run; the clause loop at edges.rs:139-168 emits multiple
per-column compares correctly. OPEN CONTRADICTION (the actual fix target): a prior
session saw ZERO calls to `lower_guard_conjunction`, so the splitter falls through
producing nothing. Prime suspect: `lower_guard_conjunction` returns EMPTY because its
`plan.guard_for_dispatch(source_dispatch_index, edge.order)` key (dispatch_index +
`edge.order`) doesn't match the build-time key (`state.dispatch_index` +
`statement_order`); OR `RuntimeDispatchLoopEdge.guard_lowering` was copied such that
`guard_can_emit_directly` is true and skips the splitter.
NEXT (fix, ~1-2 instrumented iters): eprintln in edges.rs
`select_dispatch_guard_instructions` printing `guard_can_emit_directly(edge)`,
`edge.guard_lowering`, `edge.order`, and `lower_guard_conjunction(...).is_empty()`
for the `route` edges; reconcile the lookup key so the And decomposes into per-column
clauses; THEN add a loud error in the emission filter
(`omega-machine-emission/src/layout.rs` ~L97 + `instruction_bytes.rs` ~L166) so a
zero-width EvaluateDispatchGuard can never silently vanish again. Repro: build+run
`canaries/pass/dungeon/runtime_direct_boolean_conjunction_exit` (want 21). CLI bin is
`omega` (`target/debug/omega.exe`), NOT `omega-cli`. (`lower_guard_conjunction`/
`lower_guard_leaf` themselves are correct — they DO decompose And; the gap is purely
why edges.rs gets no clauses from them.)

1. **Dispatch-guard zero-width fix (~7 wrong-exit canaries).** REPRO CONFIRMED:
   `cargo run -p omega-cli -- canaries/pass/dungeon/runtime_direct_boolean_conjunction_exit/main.omg`
   then run `.../build/omega-program.exe` → exits 10, want 21. Root cause (full
   detail in the "Wrong dispatch branch selection" bullet below): guards drop to
   `NeedsRuntimeExpression` because `resolve_guard_operand_layout`
   (`omega-state-guards/src/operands/layout.rs`) returns `None` for the
   `self.field` scrutinee columns; emission then silently emits zero bytes for
   them. NEXT STEP: instrument `lower_guard_leaf`
   (`omega-state-guards/src/conjunction.rs` ~L234) to print the operand
   kind/storage/byte_offset + resulting lowering, run the freshly-built
   `target/debug/omega-cli.exe` directly, confirm WHY operand layout is `None`
   (suspect `path_targets_source_machine` / `field_layout_by_symbol_or_name`
   failing to match the machine field for multi-column ordered transitions), then
   fix it there. Also add a loud error in the emission filter
   (`omega-machine-emission/src/layout.rs:97` + `instruction_bytes.rs:166`) so a
   non-`CompareStaticValue` guard fails instead of silently emitting zero bytes.
2. **E2 x86_64 runtime value operand + line-read encoder (5 canaries + makes E3's
   stdin PEs actually run).** `X86_64 runtime value operand is not implemented yet`
   on `runtime_machine_owned_indexed_integer_write`,
   `runtime_mutable_local_indexed_parameter_write`,
   `runtime_nested_subslice_dynamic_index`, `runtime_slice_index_read`,
   `runtime_slice_index_read_dispatch`; plus `encode_runtime_text_line_read`
   returns `unsupported_x86_64_encoding` (width/offset helpers return 0) so stdin
   PEs emit zero bytes and segfault (exit 139). Port from the working aarch64
   impls in `omega-isa-aarch64` / `omega-instruction-selection`. (A read-only E2
   diagnosis agent was launched then stopped at session end before reporting —
   re-run it or implement directly from the aarch64 reference.)

Lower priority backend: D (dungeon blank-text render — string/text descriptor
materialization; a diagnosis agent was launched+stopped before reporting) and Sd
(generalize subslice descriptors beyond literal fixed-array). Then the solo
operator-resolution consolidation (Operators section).

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
- [ ] Add focused pass/fail canaries for each newly supported subslice descriptor
  lowering shape as it becomes real.
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
- Current local suite status: `cargo test -p omega-compiler --test canary_suite`
  passes all active canaries, with no registered pending canaries (the proofs
  false twins were promoted to `fail/proofs/` by the entailment engine; see
  `wiki/proof_engine_roadmap.md`). Keep this line current when backend/runtime
  work moves canaries between `pass`, `fail`, and `pending`.

### Docs

- Add a dedicated guide section/chapter for core semantic types once syntax
  stabilizes; add navigable core docs alongside `omega/language/core`.
- Keep traits/modules/host-boundaries sequencing coherent; keep speculative
  topics clearly labeled as working direction.
