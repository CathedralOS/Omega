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

## Outstanding (pick up next)

Snapshot after the 2026-06-10 wave (decisions 8/9/10 implemented; suite
179/179, oracle fully matched). Ordered roughly by leverage.

**Decisions needed (sign-off register, 2026-06-12).** Every vertical slice
below is complete; what remains is gated on these maintainer calls. Each
points at the bullet carrying the full proposal:

1. **`Versioned<T>` container** — DECIDED 2026-06-12 (frozen decision 14):
   permanent builtin template type; u32 era; union-of-eras payload storage;
   `era` read-only source-queryable; incomplete-chain = report verdict, not
   error; paren arm form binds the whole historical value
   (`Counter::v1(old) ->`). Stage 3a/3b unblocked.
2. **Argumented ranking-view spelling** — DECIDED 2026-06-12: the use-site
   subtraction (`decreases limit - index`) is rejected as permanent surface;
   BUILD the argumented view `decreases (index, limit) ->
   Nat::BoundedDistance` (tuple form: the arrow's left side is uniformly
   the ranked subjects) and RETIRE the subtraction spelling once it lands.
   See the Measures bullet for the grammar-surgery scope.
3. **Call-output borrows** — DECIDED 2026-06-12 (frozen decision 15): adopt
   the RUST MODEL wholesale — lifetime parameters with the tick spelling
   (`machine header<'buf>(buffer: &'buf [u8], ...) -> &'buf string`),
   aggressive elision (one ref input → output borrows it; `&self` → self),
   borrow-carrying data IN-MODEL (`data ChatMessage<'buf>`), descriptive
   lifetime names as house style. Unblocks zero-copy wire decode +
   view-returning machines.
4. **Long-view arc priority** — RESOLVED 2026-06-12: all four arcs were
   scouted in parallel; the briefs live in wiki/design_briefs/ and their
   maintainer decisions are the register below.

**Decisions needed (scout round 2, 2026-06-12).** Four design briefs in
wiki/design_briefs/ (concurrency_atomics, separate_compilation, comptime,
allocator_story). Each question one line + the scout's recommendation;
sign-off freezes them.

CONCURRENCY (briefs/concurrency_atomics.md):
- C1 DECIDED 2026-06-12 (supersedes the scout's `yields` proposal): NO
  suspension keyword and NO await. Waiting originates ONLY at boundary
  wait primitives (a `Scheduler` boundary trait: host targets bind
  futex/WaitOnAddress syscalls; Cathedral userland binds the scheduler
  capability; the Cathedral kernel implements it over hlt/interrupts).
  `suspend` is an INFERRED transitive effect (decision-12 machinery),
  declarable on signatures and checked like any effect; awaiting = calling
  (the task parks inside the callee; frames are planned storage, so a
  parked task is just data — no Future reification needed). Enforcement,
  not vigilance: borrows may not live across a suspend-effect call site;
  effect ceilings forbid `suspend` where parking is illegal (ISR
  contexts); atomicity is DERIVED (a state calling no suspending machine
  runs uninterrupted). Artifacts surface all suspension points.
  Follow-on decisions from the same discussion (2026-06-12):
  - C1a SCOPED SPAWNS, no keyword: the lexical block IS the scope. A spawn
    borrowing parent locals holds ordinary loans, so the join must occur
    before the block ends; dropping a `Join<T>` JOINS (blocks), so an
    unconsumed handle joins implicitly at scope end. Free-floating spawns
    stay move/copy-only. DECIDED.
  - C1b TASK STORAGE: no stack sizes exist — no general recursion + planned
    frames mean the compiler computes each spawned machine's EXACT
    worst-case storage M; pools are per-machine-type M x N slots (declared
    N; overflow is a proof obligation or boundary failure). Region-backed
    dynamic N later (allocator arc). Overflow-impossible by construction.
    DECIDED.
  - C1c ATOMIC-STATE GUARANTEE is derived and documented precisely: a state
    body that calls no suspending machine cannot have ITS TASK parked
    mid-body. It is NOT mutual exclusion (other tasks run on other cores;
    cross-task safety = ownership/[send]/atomics). The language stays
    scheduler-agnostic (Cathedral may preempt; guarantees come from
    ownership, not non-preemption). DECIDED.
  - C1d CANCELLATION IS A VALUE AT THE WAIT (proposed, pending ch15
    alignment): no unwinding exists, so a cancelled scope makes each
    child's current/next wait return the zero case (`Cancelled`) instead
    of ready; the machine transitions to its own cleanup path and drops
    run as frames retire. Never interrupts mid-state. A never-suspending
    task is joinable but not cancellable (its effect surface says which).
    Cancellation rides the SAME propagation channel as ch15 recoverable
    errors, whatever that lands as.
  - C1e WAITABLE SURFACE IS FUTEX-SHAPED AND SINGULAR: one primitive
    (wait on word / wake N) — mutex, condvar, channel, join, timer are
    library above it; interrupts and IO completions POST TO WORDS. The
    anti-Linux-sprawl rule: no second wait mechanism, ever. DECIDED.
  - C1f SELECT DISSOLVES: no select construct. Multiplexing is data-level —
    producers post into ONE mailbox carrying a case-bearing sum
    (`Event { case Packet(...); case Tick; ... }`), the consumer does one
    wait and one ORDINARY transition over the sum (Erlang's one-mailbox
    model; already Cathedral's IPC-ring shape). Deferred work shrinks to a
    core MPSC event-queue library on the wait primitive. DECIDED.
- C2 Unit of concurrency: spawned machine = one task, per-task frame
  discipline now (separate-compilation-ready). REC: yes.
- C3 Cancellation: structured Join SCOPES (scope drop cancels children,
  deadlines attach to scopes). REC: yes.
- C4 Sharing: atomics-only at language level; `Mutex<T>` is a core-library
  type over atomic spin-locks, never a primitive. REC: yes.
- C5 Atomics + model: compiler intrinsics, five C11 orderings, C11 memory
  model wholesale. REC: yes.

SEPARATE COMPILATION (briefs/separate_compilation.md):
- S1 Component = PACKAGE; artifact = sealed IR + boundary manifest +
  layout/wire reports (.o format follow-up). REC: yes.
- S2 Linking: hermetic static composition phase first; loader-time
  relocation deferred to Cathedral's loader. REC: yes.
- S3 Cross-package monomorphization: REJECT in stage 1, resolve at
  composition time in stage 2. REC: yes.
- S4 Cross-component ABI: compiler-ENFORCED public layout reports +
  wire-data contracts for evolution edges; host ABI reused for calls.
  REC: yes.
- S5 Dispatch: keep ONE fused loop with per-component entries + import
  tables (never split per component). REC: yes.
- S6 The composition/linker tool is OMEGA's (Cathedral consumes it).
  REC: yes.

COMPTIME (briefs/comptime.md):
- M1 Purity gate: reuse decision 12's inferred transitive effect surface
  (empty effects + no &mut/out = const-evaluable). REC: yes.
- M2 Reflection access spelling: bracket form `self.[field]`;
  `Self::fields` exposes names + types only in stage 1. REC: yes.
- M3 Termination: NO new rule — const-evaluable machines inherit the
  language's existing termination discipline (general recursion does not
  exist; self-calls are tail self-loops; loops carry decreases/measures).
  Fuel at most as a defense-in-depth backstop against checker gaps.
  (Maintainer-corrected 2026-06-12; the scout's self-recursion framing was
  Rust-shaped.) REC: yes.
- M4 First const position: fixed-array lengths; TARGET-width emulation in
  the const evaluator is mandatory from day one. REC: yes.
- M5 Generator bodies must expand to effect-free machines (build-time code
  is declarative only). REC: yes.
- M6 equatable.rs is TEMPORARY: stage 2 rewrites Equatable as a core trait
  generator and retires the hand-rolled path. REC: yes.

ALLOCATOR (briefs/allocator_story.md):
- A1 No ambient heap ever; allocation is an explicit capability. REC: yes.
- A2 The allocator surface is named `Region<'r>` (over `Arena`), bound
  through the frozen `Allocation` provider category. REC: yes.
- A3 Failure semantics: proof-obligated capacity (`requires len <
  capacity`); `try_push -> Result` optional later; no silent traps.
  REC: yes.
- A4 Vec ladder: stage 1 fixed-capacity (no allocator at all); stage 2
  `Vec<'r, T>` borrows a Region, capacity fixed at construction, NO
  growth; pluggable allocators only if demand appears. REC: yes.
- A5 Drops: elements drop immediately; the Region frees memory in bulk
  (cleanup and memory release are separate concerns). REC: yes.

Smaller wire remainders (repeated fields, arbitrary-depth nesting,
encoding families, negotiation) are derivable from decision 10 + the
landed framing without sign-off. Language-design open questions with no
implementation pressure stay in the guide's appendix "Still Open".

**Implementation, design already frozen:**

All three frozen decisions (11, 12, 13) landed 2026-06-11 — see the wave
notes under Next Up. Decision 11's formerly-accepted hole (place==place on
a payload-bearing sum slipping through as a tag/width compare) is now
CLOSED for typable operands by Equatable synthesis: conforming types expand
structurally, non-conforming structural types error with the conformance
suggestion (operands the state typing scope cannot type — e.g. inside
contracts — still slip through). Decision 13's residue (machine-call
monomorphization arguments not bound-checked; generics-completion arc)
remains tracked in its bullet below.

- [ ] **Lifetimes (decision 15).** New implementation arc: `'name` lifetime
  parameters in the `<>` generic list (lexer tick token, parser, all three
  tree representations), elision rules (one ref input → output borrows it;
  `&self` → self), borrow-checker linkage (returned view extends the named
  input's loan), borrow-carrying `data` declarations. Staging suggestion:
  elision-only first (no user-visible ticks; fixes the conservative
  all-args aliasing), then explicit parameters, then struct borrows.
  Unlocks zero-copy String decode + view-returning machines.
- [ ] **Ranking-view spelling (decision 2 above).** Build
  `decreases (index, limit) -> Nat::BoundedDistance`; retire the use-site
  subtraction form once landed. Grammar scope in the Measures bullet.
- [ ] **Wire stage 2: encoders + decoders.** STAGE 2a LANDED (2026-06-11):
  era assignment along the version chain (decision 10; queryable on the
  typed `WireSchema`, surfaced in `04_wire_protocols.txt`), the synthesized
  `Schema::encode_wire(&value, &mut out, &mut written)` encoder for
  primitive integer fields (i32/i64/u32/u64/bool; other types reject), and
  compact_binary v0 framing (era varint, then per field a tag varint +
  value varint; LEB128, zigzag for signed, bool 0/1) -- lowered as two
  dedicated wire-append operations on BOTH aarch64 and x86_64 (cursor lives
  in the `written` slot; widths/relocations in pinned lockstep), with
  byte-identical native interpreter support and byte-exact run canaries in
  the differential oracle. STAGE 2b LANDED (2026-06-11): the current-era
  decoder `Schema::decode_wire(&mut value, &buffer, &mut read, &mut ok)` --
  expected-byte reads for the era discriminator and field tags plus a
  bounds-checked LEB128 value read per field (un-zigzag for signed), as two
  dedicated wire-read operations on BOTH ISAs (cursor in the `read` slot,
  STICKY failure flag in the `ok` slot: wrong era / unexpected tag /
  truncated / overlong varint fail cleanly, every read bounds-checked
  against the buffer's compile-time length; widths/relocations pinned),
  interpreter parity including the failure path, and round-trip +
  wrong-era-rejection run canaries in the differential oracle. STRING
  FIELDS, ENCODE-ONLY, LANDED (2026-06-11): a String field rides as tag
  varint + LENGTH varint (byte count) + raw UTF-8 bytes (no NUL, no
  padding), lowered as one new `AppendWireTextBytes` operation on BOTH ISAs
  (loads the `{ptr, len}` text descriptor, reuses the scalar LEB128 emit
  loop for the length, then a byte-copy loop that bounds EVERY store
  against the out buffer's compile-time capacity and drops overflow --
  widths/relocations pinned, byte-exact run canary in the differential
  oracle). Validation allows at most ONE String field and requires it to
  carry the highest field number (it encodes last) so every earlier append
  keeps the compile-time worst-case capacity guarantee; the worst case
  budgets the String's tag + ten-byte max length varint. String DECODE
  stays rejected -- the honest options were (a) zero-copy (descriptor
  pointing into the decode buffer) or (b) reject, and we took (b) because
  today's borrow facts only track view loans from explicit borrow
  expressions: the checker cannot see `decode_wire(&mut value, &buffer,
  ..)` leaving `value`'s String field aliasing `buffer`, so buffer
  mutation after a zero-copy decode would silently invalidate the decoded
  string -- a KNOWN HOLE to close before (a) lands (borrow-facts follow-up:
  model a call output retaining a borrow of another argument; RULED
  2026-06-12, frozen decision 15: the Rust lifetime model is adopted, so
  zero-copy String decode is mechanical once lifetimes are implemented:
  read len varint, bounds-check against the remaining buffer, store
  `{buffer_base + cursor, len}`).
  Encode also has no runtime overflow signal (content past capacity is
  dropped; callers size buffers for their longest text) -- an encode
  ok/overflow out-parameter is candidate follow-up work. NESTED MESSAGE
  FIELDS LANDED (2026-06-12), one level deep, scalar-only child bodies: a
  field whose type is a sibling wire schema rides as tag + byte-LENGTH
  varint + the child's tag/value pairs with NO era discriminator (decision
  10: one era varint per top-level message, never per struct). The actual
  length is runtime-sized (varints), so the encoder two-pass STAGES the
  sub-message through a planner-reserved frame scratch region shaped as a
  `{ptr, len}` text descriptor + worst-case staging buffer, then replays it
  through the existing `AppendWireTextBytes` (length varint + bounded copy)
  -- ZERO new encode operations; capacity math composes (parent worst case
  counts tag + length varint + child worst case), and the one-String-LAST
  rule is per message scope (child bodies have no String today). The
  decoder reads the length into the scratch slot, then two new loop-free
  operations on BOTH ISAs (widths/relocations pinned):
  `ReadWireNestedOpen` (absolute end bound = cursor + length, checked both
  as raw length and as bound against the buffer so a huge length cannot
  wrap the 64-bit sum) and `ReadWireNestedClose` (sticky ok fails unless
  the cursor lands EXACTLY on the bound). Schema cycles (no finite worst
  case) are hard errors at the declaration
  (wire/nested_schema_cycle); String-in-child and nested-in-nested reject
  at the call (wire/encode_nested_in_nested); round-trip + corrupted-length
  run canaries with hand-computed bytes in the differential oracle
  (wire/runtime_wire_roundtrip_nested_exit,
  wire/runtime_wire_decode_rejects_bad_nested_length_exit), interpreter
  parity included.
  Remaining: historical-era decode via `Versioned<T>` (after the stage 3
  sign-off), String decode (above), arbitrary-depth nesting (needs
  per-level staging regions), repeated fields,
  wire-schemas-as-program-types, runtime layout of wire values, encoding
  families beyond compact_binary v0, version negotiation. (Found while
  landing, FIXED 2026-06-11: struct-literal String field initialization did
  not lower to a native descriptor write -- data planning never collected
  string literals from `let` local initializers, so the descriptor-write
  selection found no data object and silently skipped; pinned by
  data/runtime_struct_literal_string_field_exit, which covers the record-
  and case-literal forms.)
- [ ] **Versioned data stage 3.** Era tag + the wire integration decision 10
  assumes; era-tagged containers that make version MATCH arms selectable
  (stage 2 ruled them unreachable — no value can hold a historical era yet);
  migration chains, `replaces`, quiescence obligations. (Stage 2 landed
  2026-06-11: historical-shape construction, the type-name migration call,
  the first runtime migration canary, struct-literal field validation.)
  DESIGN SIGNED OFF 2026-06-12 (frozen decision 14): builtin `Versioned<T>`
  — `{ era: u32, payload: UNION-OF-ERAS }` — constructed at boundaries only
  (chapter 21: ordinary values never carry era tags); version match arms
  legal ONLY on `Versioned<T>` subjects (tag compare + shape
  reinterpretation per arm; paren form binds the whole historical value);
  `era` read-only source-queryable; incomplete-chain = report verdict; the
  wire decoder is NOT a prerequisite. Stage 3b (no new surface,
  dispatchable independently): migration-chain completeness validation
  along the declared version chain. `replaces`/quiescence stay deferred
  behind the concurrency model.
- [ ] **Equatable synthesis / conformance defaults.** EQUATABLE SYNTHESIS
  LANDED (2026-06-11): `Type satisfies Equatable;` on a record or
  payload-bearing sum makes `==`/`!=` legal -- expanded INLINE at
  resolved->typed lowering into field compares (sums: OR over cases, tag
  compares first, then payload fields), riding existing backend/interpreter
  comparison machinery; the interim `==` error is retired for conforming
  types and extended with a declare-the-conformance suggestion for
  non-conforming ones; a written `Type::equals` wins (`==` lowers to a
  call); prerequisites error at the conformance item (every field scalar /
  `String` / payload-less sum / conforming; recursive types rejected). The
  interpreter short-circuits `&&`/`||` and ZII-defaults enum fields to the
  zero case; the native value-operand resolver reads oversize enum places
  as their tag prefix in tag compares (was a silent statement drop for
  two-field payloads). STRING FIELDS LANDED (2026-06-11): a `String` field
  compares by CONTENT through a new `TextEquals` value-operand LEAF
  (`{left, right}` descriptor places -> bool) lowered in both ISAs as a
  length compare plus a bounded byte loop (fixed-width encodings, pinned
  left/right descriptor-base relocation offsets, debug_asserts against the
  width functions); selection routes `String == String` place compares to
  it in nested-operand AND top-level binary-write positions; comparing a
  String field against a CONSTRUCTED LITERAL stays rejected (no stored
  descriptor at the compare site -- bind it to a value first). Canaries:
  pass+RUN `traits/equatable_record_equality_exit` +
  `traits/equatable_sum_payload_equality_exit` +
  `traits/equatable_string_field_equality_exit` (equal contents / same
  length different bytes / different lengths / scalar sibling), fail
  `traits/equatable_missing_conformance_suggested` /
  `equatable_field_not_equatable` / `equatable_recursive_type` /
  `equatable_string_field_literal_compare`. STILL OPEN: a CALLABLE
  synthesized `Type::equals` machine (comptime/trait-generator arc), trait
  `default machine` instantiation for other traits, recursive Equatable
  support, String-vs-literal structural compares, equality in
  contracts/domain facts (no typing scope there), and written-equals
  signature matching against `&Self` (validation accepts `Self` in trait
  signatures; substitution per conformance is unchecked).
- [ ] **Case members: remaining halves.** EXHAUSTIVENESS COUNTING LANDED
  (2026-06-11), over implicit case-domains AND case-subset domains: a
  dispatch run (consecutive transitions, the shape every block desugars to)
  whose arms classify a case-bearing subject must cover every case or close
  with `_`. Decidable arms: case arms (one tag) and PURE case-union domain
  arms; predicate-domain arms, `if`-guarded patterns, and value compares are
  uncountable, so uncovered+uncountable errors suggest `_`, while fully
  counted gaps name the missing cases ("match over `Command` does not cover
  `Command::Move`; add an arm or `_`"). RULING (chapter-1 footnote): pure
  case-union recognition is SYNTACTIC -- the domain's `when` classifier must
  be literally `self in Type::A | Type::B` over its own target type's cases
  with NO other facts; classifier analysis stays a possible later widening.
  The check runs on RESOLVED trees (omega-symbol-resolved-trees-to-typed-
  trees/src/exhaustiveness.rs, the `crate::equality` pattern) because typed
  lowering erases membership into tag compares/classifier expansions. With
  it landed: `when` classifiers now admit membership unions, `domain T::D
  when ...;` (semicolon, body-less) parses, and executable declared-domain
  membership now ANDs the classifier into the test (a union-subset domain
  works as a guard/arm at runtime; native+interpreter agree, see
  pass/data/match_exhaustive_by_case_union_domain). Probe record: before the
  check, a 2-of-3-case dispatch compiled and FELL THROUGH divergently at
  runtime (native exit 1, interpreter exit 0) -- the error is the fix.
  Corpus fallout: ZERO (suite was already covered-or-defaulted). Canaries:
  fail data/match_nonexhaustive_cases +
  data/match_predicate_domain_needs_default; pass+RUN
  data/match_exhaustive_by_cases + data/match_exhaustive_by_case_union_domain;
  pass data/match_default_satisfies_exhaustiveness. Payload sums are done;
  `self in Type::Case` and unions at use sites landed with decision 11.
  MIXED SHAPES LANDED (2026-06-11) -- the final half of decision 7; both
  halves of this item are now closed (see the next entry).
- [x] **Mixed data shapes (common fields + case part) LANDED (2026-06-11).**
  Decision 7's final half; the trees already modeled fields+cases together
  (only validation rejected). Decisions recorded here:
  - LAYOUT (owned in omega-layout, `DataShape::Enum` now carries
    `common_fields`): TAG-FIRST -- tag at offset 0, common fields packed
    after the tag, payload overlay after the common fields. Deliberate
    deviation from the suggested common-fields-first order: the backend's
    tag-only compares/writes (state-guard clamps, runtime value operands,
    static folds) treat "first ENUM_TAG_BYTES of the value" as the tag
    WITHOUT layout context, so the tag offset must stay the universal
    constant 0. Common-field offsets are case-independent constants in
    either order; ZII holds (zeroed value = first case + zeroed common
    fields); pure sums degenerate to the historical layout (empty common
    span), so every existing offset is unchanged.
  - CONSTRUCTION: case-literal form only (`Type::Case { ... }`; record-form
    literals over case-bearing types are rejected). Common fields may be
    named alongside payload fields; every common field NOT named
    ZERO-INITIALIZES (explicit zero writes ride the ordinary member-write
    path natively; the interpreter zeroes the cells), because construction
    replaces the whole value. Consequences, both hard errors: common-field
    defaults (would silently never apply) and non-scalar common fields
    (first cut: zeroing nested aggregates/text at construction is deferred).
    Payload-field names may not collide with common-field names (member
    access searches both).
  - ACCESS: common fields read/write WITHOUT case knowledge
    (`event.consumed` / `event.bonus = 5`); payload fields stay case-bound.
  - EQUALITY: Equatable over mixed = common fields AND tag AND matching
    payload (equatable.rs Mixed -> Structural; structural_equality.rs
    conjoins common compares with the sum expansion). FOUND+FIXED a latent
    compiler hang: omega-state-values folding's `factor_common_conjuncts`
    re-entered `boolean_and`, whose distribute-over-Or rewrite re-created
    the factored shape -- non-terminating mutual recursion, first reachable
    via mixed equality (its arms share the common-field compares). Factoring
    now re-attaches conjuncts with a non-distributing combinator.
  - REJECTED LOUDLY (scope kept honest): wire `encode_wire` over ANY
    case-bearing value type (sum or mixed) -- the schema field set has no
    spelling for the tag/payload, so encoding would silently drop the case
    part (this also closed a pre-existing silent hole for pure sums).
    Unnamed common fields in equality-compared case literals keep the
    existing "literal omits field" diagnostic (name the field).
  - Exhaustiveness, tag dispatch, payload binding, and `in` membership work
    over mixed unchanged (tag@0 preserved every existing path). Canaries:
    pass+RUN data/runtime_mixed_shape_exit (construct with named common
    field, case change zeroes unnamed common field, common write, 3-case
    dispatch with payload binding, exit 70) +
    traits/equatable_mixed_shape_equality_exit (common-field-only
    difference compares unequal), both differential; fail
    data/mixed_common_field_nonscalar, data/mixed_common_field_default,
    data/mixed_payload_field_shadows_common, data/mixed_record_literal,
    wire/encode_case_bearing_value. Retired:
    fail data/mixed_data_shape_unimplemented.

**Backend residue (small, known):**

- [x] Eager-guard divergence (effectful transition SUBJECTS) FIXED: a guard
  subject like `transition self.should_carve(random, 2) { true/false }` now
  evaluates exactly ONCE natively, matching the interpreter, even with
  diverging arm targets and a nested callee chain. Three compounding causes,
  all repaired: (1) every arm's guard holds a parser COPY of the subject call
  and each arm allocated its OWN `__call_result` slot — the runtime-storage
  plan now shares ONE slot across arms with structurally equal subjects
  (`shared_transition_guard_slot_offset`, omega-runtime-storage/body.rs);
  (2) later arms appended their own nested-callee/leaf/straight-line
  expansions, re-running the callee's side effects per arm — the
  runtime-branching plan now suppresses ALL execution machinery for repeated
  subjects (omega-runtime-branching/branching/mod.rs + expansions.rs);
  (3) `let x = self.f(...)` inside an expansion emitted the call TWICE (once
  for its StateCall operation, once via the LocalData operation's
  initializer-call path) — one doubling per nesting level, the dungeon's
  32-draws-for-1 amplification (instruction-selection straight_line.rs).
  Regression net: canaries/pass/control_flow/
  runtime_effectful_subject_single_evaluation_exit (diverging-arm,
  3-deep chain; pre-fix native exits 77, post-fix 70 = interpreter) plus the
  measured dungeon shape (1 draw per should_carve decision in BOTH backends).
- [x] Non-guard call chains over-draw / read stale values natively — BOTH
  named symptoms FIXED (2026-06-11); the splice is now the single executor of
  record for non-guard chains. (1) OVER-EXECUTION: the
  `carve_room -> roll_event -> rng.range -> next_u32` STATEMENT chain ran
  `next_u32` 3x natively (interpreter 1x). The three executors, mapped in the
  backend report: the splice's flattened Mutation op (the keeper), the
  non-guard branch PRELUDE's StateCall arm (a `let x = self.f(...)` statement
  classifies as StateCall in prelude_operations, and its arm re-emitted the
  callee's nested expansions), and the nested-walk straight-line expansion
  (created by append_branch_prelude_expansion's callee walk, then matched
  AGAIN at the flattened nested call's own body op). Plan-level suppression
  mirroring the eager-guard fix: non-guard (LocalDataOnly) preludes now carry
  ONLY call-free local initializers (omega-runtime-branching operations.rs)
  and never walk nested callees (expansions.rs — only guard-role `All`
  preludes walk, since the splice flattens every nested call into the body
  where each gets directly-matched machinery). (2) STALE READ: depth-1
  `let v = self.next(&mut state)` returned the PRE-mutation value because the
  call-result value selection (leaf expansion) emitted at the StateCall body
  op, before the splice's mutation ops. The dispatch loop now DEFERS the
  selection to the statement's own LocalStorage operation (after the callee's
  spliced effects, before the local copy) when the statement's only leaf role
  is AssignmentValue (instruction-selection runtime_dispatch.rs + leaf.rs
  `leaf_expansions_defer_to_local_initializer`). Canaries: pass+RUN
  control_flow/runtime_statement_call_single_execution_exit (pre-fix native 3
  = three executors, post-fix 70) and
  calls/runtime_assignment_call_post_mutation_value_exit (pre-fix native 2 =
  stale read, post-fix 70), both in the differential oracle. Dungeon: seed-7
  generation went from 34 native draws to 14 (interpreter 15).
- [x] Dungeon residual, ONE draw — FIXED (2026-06-12). The misfire was NOT an
  arm-selection/flow bug and NOT a stale depth: delta-debugging the dungeon
  down to a 130-line skeleton (copy sample to /tmp, delete rooms/events/
  systems while native!=interp held) plus an lldb breakpoint trace of the
  emitted guard loads showed the second `roll_event`'s parameter slot
  receiving 0xFFFFFFA6 = -90: the inline CALL ARGUMENT `raw % 100` (raw: u32,
  a prior call result) was emitted with SIGNED division (sdiv 0x1ada0e33),
  and for raw >= 2^31 the negative remainder reads as a huge value under the
  ladder's UNSIGNED guards (`roll < 20`/`roll < 60` both fail), falling into
  the enemy arm whose bat draw (depth 1 — legitimately <= 1, hence the
  "stale depth" misread) advanced the stream once. The first call survives by
  luck (its raw < 2^31), which is why the bug needed two call contexts; small
  probes passed because their raw values never crossed 2^31. Root cause:
  `select_runtime_storage_binary_write_in_table` (the pre-resolved-place
  entry the frame-slot ARGUMENT write funnels through) never ran the
  signedness adjustment its sibling targeted-mutation path has — fixed by an
  operand-only `signedness_adjusted_operator_for_operands` (binary_table_
  writes.rs); the branch-expansion binary write (branches/mutation.rs), a
  third drifted copy, now adjusts too. Selection-level only; aarch64/x86
  widths for Modulo vs ModuloUnsigned are identical. Canary: pass+RUN
  arithmetic/runtime_unsigned_modulo_call_argument_exit (pre-fix native 71 =
  4 draws, post-fix 70 = 3 draws = interpreter), in the differential oracle.
  Dungeon seed-7: full-tour event/path lines now byte-match the interpreter
  across all eight rooms (draw streams agree; the bullet's "14 vs 15" is
  retired). R05's description stayed un-asserted for a DIFFERENT reason — the
  side-room carve guard, since resolved (next bullet); both side-room
  description lines are now asserted in the scripted suite test.
- [x] Side-room DESCRIPTIONS lost natively — RESOLVED (2026-06-12). The
  suspect shape was wrong: the description WRITE machinery (carve through
  `room_mut`'s `&mut Room` in a guard-branch target) was sound and its
  selected/encoded code byte-correct per dispatch. The side rooms were never
  CARVED natively at all: `transition self.should_carve(random, N)` always
  took the FALSE arm because the guard byte was never computed. should_carve
  returns `self.rng.chance(random, chance, 100)`, and chance's inline leaf
  value `roll < numerator` binds `numerator` to should_carve's local `chance`
  (`max(15, 80 - depth*6)`), a fold-only local with NO frame slot — the leaf
  context could not resolve the name as a place, so
  `select_runtime_leaf_branch_terminal_value_write` silently emitted nothing
  and the chance call-result slot stayed 0. Every other side-room render line
  (label/event/paths) is HARDCODED per cell in the view, which is why only
  the data-driven description line exposed it (and why the "RNG streams
  match" observation held: the draws ran via the straight-line expansion;
  only the decision byte was lost). Fix: leaf terminal-value resolution now
  substitutes caller-local initializer names (bindings re-applied) for
  slot-less locals (`resolve_leaf_caller_local_initializer_names`,
  branches/leaf.rs) — selection-level only. Canary: pass+RUN
  dungeon/runtime_nested_value_call_caller_local_guard_exit (pre-fix exit 71,
  post-fix 70 = interpreter), in the differential oracle. The dungeon
  scripted suite test now detours through R06 and asserts BOTH side-room
  description lines; the full tour is byte-identical to the interpreter.
  Residue spotted while hunting: a `transition rooms[i].description ==
  "literal"` String-equality guard evaluated TRUE natively while the field
  was empty (two false-negative probes) — RESOLVED, next bullet.
- [x] Slice-indexed String guard compares lied — RESOLVED (2026-06-12).
  Failure class: SILENTLY DROPPED COMPARE, guard defaults truthy. A
  `String place == "literal"` guard (slice-indexed `items[i].name` AND plain
  fields `self.name` alike, `!=` too) had NO selection: the buffer-literal
  guard needs a runtime text buffer (stdin machinery), the storage guard
  needs places on BOTH sides, and the value guard can neither resolve a
  literal operand nor compare 16-byte descriptors — every path returned None,
  the dispatch edge emitted no compare (`EvaluateDispatchGuard
  NeedsRuntimeExpression` encodes nothing), and the first arm was taken
  unconditionally. Both probe regimes lied (empty AND non-empty-differing);
  the matching case "passed" for the same reason. Fix: a new
  `TextEqualsLiteral` value operand (place handle + inline literal bytes,
  bool 0/1; guards lower it as `CompareRuntimeValues == 1`), selected by
  `runtime_text_equals_literal_guard(_in_table)` for String-typed Storage and
  FrameIndexed descriptor places (frame-indexed tried FIRST so a slice index
  never falls back to the descriptor-as-value trap); emitters in both ISAs
  with width fns in lockstep (length mismatch short-circuits unequal, so a
  zeroed descriptor's null pointer is never dereferenced; the TextEquals
  half-empty behavior was audited and is correct). Honest guards then
  UNMASKED three double-masked write bugs, all fixed: (1) skewed relocation —
  aarch64 `runtime_machine_indexed_string_runtime_frame_address_offset` said
  20 but the encoder puts the frame adrp at 12, so machine-indexed string
  writes read a garbage index and landed nowhere; (2) concat-built String
  LOCALS (`let line = "== " + name + " =="`) were never materialized — local
  initializers are not mutations, so the runtime-text planner never planned
  their builder (StateLocalStorage now carries `initial_value`;
  `collect_runtime_text_local_initializer_writes`); (3) ALL-LITERAL concats
  (`"prefix " + "omega"`) per-segment "appends" to machine-indexed targets
  are full descriptor writes, leaving only the LAST segment — now folded to
  one StaticText write at planning/data/selection in lockstep. Canaries:
  pass+RUN text/runtime_slice_indexed_string_guard_exit (empty field takes
  the false arm, matching takes true, same-length-differing takes false;
  exit 70 only when all three behave) and
  text/runtime_string_field_literal_guard_exit (the storage-place sibling),
  both in the differential oracle. The remaining place-kind gap is RESOLVED
  (2026-06-12): TextEqualsLiteral selection + both ISA emitters now cover
  FrameBaseIndexed (local inline fixed array, frame base + runtime index
  scale), FrameFixedIndexed (slice descriptor + folded constant offset), and
  Pointee (pointer slot deref + field offset) places, widths in lockstep
  (x86_64 setup 30/17/17 bytes; aarch64 reuses the storage-read setup width
  fns). Probes pre-fix: base- and fixed-indexed selected NOTHING (silent
  truthy — empty field "equalled" the literal); pointee places lied
  DIFFERENTLY — the storage resolver saw through the reference and selected
  the POINTER SLOT's raw bytes as the descriptor, an always-false compare
  (match regime took the false arm), in both the `&mut Room` local-alias and
  called-machine-parameter shapes. Fix: descriptor-place resolution tries
  frame-indexed, base-indexed, fixed-indexed, then pointee, with static
  storage LAST (pointee-before-storage kills the pointer-slot-as-descriptor
  trap; direct base-indexed String WRITES still hard-error
  `needs runtime storage write lowering` — honest, writes go through slice
  aliases). Canaries (pass+RUN, three regimes each — empty≠literal, match,
  same-length-differ — all in the differential oracle):
  text/runtime_local_array_indexed_string_guard_exit,
  text/runtime_slice_fixed_indexed_string_guard_exit,
  text/runtime_pointee_string_guard_exit (alias + parameter shapes; also
  linux_x64 cross-emission smoke-checked for the width debug_asserts).
  Still open follow-up: the guard fallback itself still emits silence
  rather than a hard error (guard-must-select-or-error tightening). The
  array-literal initializer residue is RESOLVED (2026-06-12): `[Room {
  label: "x" }, ..]` into a local fixed array emitted rodata but wrote NO
  frame slots — and probing showed the gap was wider than the String guards
  suggested: the local-initializer mutation path had a StructLiteral arm but
  no ArrayLiteral arm, so the WHOLE initializer (scalar elements of
  `[1, 2, 3]` included) fell through to the scalar path and selected
  nothing. Fix (selection-level, writes/mod.rs): an ArrayLiteral arm in
  select_runtime_storage_resolved_mutation_write_in_mutable_table recurses
  per element through a literal-indexed target (`target[i]`), so
  struct-literal elements expand into their per-field member writes (String
  descriptors ride the landed fixed-indexed WriteRuntimeFrameString
  machinery) and scalar elements ride the static-write path. Canary
  (pass+RUN, differential oracle):
  data/runtime_array_literal_string_field_exit (two elements, distinct
  literals, runtime-indexed guards on each element's scalar sibling and
  String field plus an element-0-vs-element-1-literal cross check; exit 70).
- [ ] Signed/unsigned residue, two sibling shapes (found while shrinking, not
  yet canaried): (1) a modulo whose operand is a CAST — `((seed >> 32) as
  u32) % 199` inside a convert/value-operand chain — still picks the signed
  encoding because `resolve_runtime_storage_is_signed_in_table` cannot see
  through Cast nodes (returns None -> signed fallback); the non-table
  `select_runtime_binary_mutation_write` (writes/mutation.rs) also never
  adjusts. (2) Trailing-state STALE READS of threaded `&mut` param fields:
  a transition-guard SUBJECT read of `random.calls` in a state appended
  after build_main_hall_1 saw the post-seed snapshot (0), and a `let hi =
  (random.seed >> 32) as u32` in a state appended after build_main_hall_4
  read a seed stale by the last TWO build_segment calls — instrumentation-
  only so far, but the same one-shrink-away family; needs its own minimal
  skeleton hunt.
- [x] Signed/unsigned residue, shape (1) — CAST OPERANDS — FIXED (2026-06-12).
  `((random.seed >> 32) as u32) % 199` lowered SIGNED because
  `resolve_runtime_storage_is_signed_in_table` could not see through Cast
  nodes (None -> signed fallback). The resolver now classifies a Cast by its
  TARGET type name (storage_places.rs) — `(x as u32)` is unsigned no matter
  what `x` is — which fixes every funnel at once (guards, edges, all binary
  writes route through this one resolver). Sibling sweep in the same change:
  the NESTED value-operand Binary/min-max builders never adjusted signedness
  at all (only top-level write operators did) — the dungeon probe's inner
  `seed >> 32` emitted the arithmetic shift, masked only by the following
  4-byte truncation. All seven remaining operator-choosing sites now run the
  shared decision: value_operands.rs in-table Binary + builtin-call,
  value_operands.rs non-table Binary + builtin-call (via a new
  `signedness_adjusted_operator_for_tree_operands` insert_tree adapter),
  branches/mutation.rs nested Binary + builtin-call, and the non-table
  `select_runtime_binary_mutation_write` (writes/mutation.rs — the cleanup
  doc's [!] alias path; instrumented across the full suite + dungeon + a
  purpose-built alias-fed guarded-transition probe, it is reached 0 times,
  so the adjustment there is defense-in-depth and a canary for it cannot be
  written from surface syntax today). Canary: pass+RUN
  arithmetic/runtime_unsigned_modulo_cast_operand_exit (pre-fix native 71 =
  signed remainder -87 in the u32 slot, post-fix 70 = interpreter), in the
  differential oracle.
- [x] Stale assignment-call result when the local's slot is ELIDED — FIXED
  (2026-06-12), option (b): the storage plan no longer elides the LocalStorage
  slot when the local's initializer contains a MUTATING call (a call passing a
  `&mut` argument). One condition in `local_data_requires_storage`
  (omega-state-storage/collection.rs, new `expression_contains_mutating_call`
  walk) — the elision is an optimization and correctness gates it: with the
  slot kept, the executor-of-record deferral
  (`leaf_expansions_defer_to_local_initializer`) has its landing op and the
  call-result copy emits AFTER the splice's mutation writes (backend report
  now shows `write binary ... Add 1` then `copy @0 -> frame@0`). Canary:
  pass+RUN calls/runtime_call_result_after_splice_mutation_exit (pre-fix
  native 71 / interpreter 70, post-fix both 70), in the differential oracle;
  guard-only sibling runtime_assignment_call_post_mutation_value_exit
  re-verified green. Original report follows. The
  "trailing-state stale-&mut-field reads" instrumentation observations
  shrink to this: `let seed: u64 = self.rng.next_seed(&mut random)` (callee:
  `state.seed = state.seed + 1; transition { _ -> state.seed }` — a PLAIN
  `&mut`-param field terminal) followed by ANY consumer statement
  (`let doubled: u64 = seed * 2`) makes `seed` deliver the PRE-call value
  natively (probe guards: doubled==84 -> 70 post-mutation, ==82 -> 71 stale;
  native 71, interpreter 70). Mechanism, read from the backend report +
  lldb slot dumps: when the assignment local feeds a LATER STATEMENT's
  initializer, the storage plan elides its LocalStorage op (slots.txt shows
  only the call-result slot, no `local seed`); the deferral fix
  (`leaf_expansions_defer_to_local_initializer`) has no LocalStorage op to
  defer to, so the call-result copy (param field -> call-result slot) emits
  at the StateCall body op, BEFORE the splice's mutation ops — emission
  order is literally `copy @0 -> @8` then `write binary @0 = @0 Add 1`.
  Guard-only consumption keeps the local slot and the copy emits AFTER
  the mutation (correct), which is why
  calls/runtime_assignment_call_post_mutation_value_exit stays green — its
  local keeps a slot. Fix direction: defer the call-result selection to the
  statement's position in splice order even when the local slot is elided
  (or stop eliding the slot for &mut-param-field call results). The
  trailing-state SUBJECT-read observation (`random.calls` after
  build_main_hall_1) is consistent with this shape feeding a guard, but was
  not separately reproduced.
- [x] 3 pre-existing `_compile` canaries hang at runtime — STALE (probed
  2026-06-11): the slice-write `_compile` canaries run now (the hang was the
  x18 zeroing below) and their dispatch shape already has a runtime `_exit`
  sibling in the suite; `calls/runtime_mutable_local_parameter_write_compile`
  "hangs" by its own unconditional `true -> main()` self-loop (source
  structure, not a backend bug; its `_exit` sibling verifies the behavior).
- [x] Straight-line `main` terminal LOCALS/EXPRESSIONS don't deliver as the
  exit code — FIXED (2026-06-11). Interpreter parity confirmed first (it
  already returned 70 for all three probe shapes; pinned in
  omega-interpreter/tests/coverage.rs). Root cause: the dispatch terminal's
  return-value selection (`select_runtime_dispatch_return_value`,
  runtime_dispatch/edges.rs) only handled a CONSTANT terminal
  (`static_terminal_target_value`) and silently fell through otherwise. Now:
  (1) constants write the immediate as before; (2) runtime places (field
  read-backs, locals with frame slots — reassigned locals always have
  storage) load via the new `CopyRuntimeStorageToReturnRegister` instruction
  (both ISAs, widths in lockstep, region-symbol relocation at instruction
  start); (3) storage-less locals/constant arithmetic constant-fold through
  `simplify_state_expression` to a small fixpoint. Residue: a runtime
  ARITHMETIC terminal (`self.n + 1`) still has no return-value write — fold
  it into a local or field first. Canaries:
  control_flow/runtime_straight_line_terminal_local_exit,
  control_flow/runtime_straight_line_terminal_field_readback_exit, and the
  promoted slices/runtime_mutable_slice_element_write_straight_line_exit
  (formerly _compile; now writes through the slice view and exits on the
  read-back), all RUN at 70 + registered in the differential oracle.
- [x] aarch64 runtime convergence (dungeon hot-potato). ROOT CAUSE FOUND AND
  FIXED: the aarch64 encoder used x18 as a general scratch for frame-slot
  copies (`ldr x18, [src]; str x18, [dst]`), but x18 is the reserved platform
  register on Darwin arm64 and XNU ZEROES it on every kernel->user return — any
  timer interrupt landing between the load and the store silently replaced the
  copied value with 0. In the dungeon this zeroed a threaded `&mut Level` arg
  (build_segment's level param), so `room_mut` computed `0 + element_offset`
  (the segfault on `str w17, [x16]` with x16 = 0x1d0 = rooms[2]'s byte offset:
  an offset-LIKE value because the BASE was the zeroed pointer). Looked
  nondeterministic/hot-potato because the first timer tick lands at a roughly
  fixed point in the deterministic instruction stream, and any debugger
  perturbation moved it. Fix: x26 (verified unused) replaces x18 everywhere in
  omega-isa-aarch64; register-only substitution, instruction widths unchanged.
  Regression net: canaries/pass/dungeon/runtime_threaded_mut_arg_interrupt_soak_exit
  (50M pointer-threaded increments across many timer ticks; pre-fix encoder
  fails it 4/5 runs, post-fix deterministic exit 70).
- [ ] Borrow layer records free-machine value-call targets as `invalid` in
  checked trees (cosmetic today).
- [x] Borrow layer records free-machine value-call targets as `invalid` in
  checked trees: fixed by the call-requires soundness wave (receiverless
  free-machine targets now resolve to the entry state in symbol resolution,
  and the checked-trees resolver accepts them).
- [x] Platform state-signature `requires` (calls through platform-typed
  contained objects) are never collected as call obligations -- the same
  vacuity the free-machine/boundary-trait wave fixed, third shape. FIXED:
  platform entries now parse the shared bodyless-signature clause grammar
  (`effects`/`requires`/`ensures`, previously a parse error), the
  checked-trees call-target resolver accepts platform state-signature
  symbols, `contract_target_from_state_symbol` maps them to the owning
  platform, and `call_target_parameters` reads the signature's parameter
  list -- so the existing instantiation path, caller-requires discharge,
  and mutation invalidation work identically to the trait shape (probe
  verified all three). Corpus fallout: none (suite stayed 187/187 before
  the new canaries; all canary `platform/console.omg` shims are boundary
  traits and were already enforced). New canaries: fail
  domains/call_requires_platform_unproven, pass
  domains/call_requires_platform_satisfied_by_caller_requires.
- [x] Stale test fixtures repaired: lib-test fixtures of omega-graph/types/
  names/proof/syntax-trees/abstract-operations/target-operations/facts gained
  the missing `abi`/`type_parameters`/`kind`/`properties`/`is_float` fields;
  omega-state-calls fixtures moved off the retired bare-`->` explicit-state
  syntax (omega-machine-emission already passed); architecture_boundaries
  brought in line with the omega-architecture-test layering policy + the
  facts/effects relocation (dev-deps exempt, pipeline->backend-helper edges
  tolerated, final machinery still forbidden, stale `lowering/` path fixed).
  `cargo test --workspace` is green apart from aarch64 MVP encoder gaps.

**Long view (deliberately deferred — big designs or revamps; listed so they
stay visible, not because they're next):**

- [ ] **Concurrency model.** Chapter 17 is a sketch; every target declares
  `threads = disabled`, zero canaries. Needs the hard answers first:
  scheduler suspension across ticks, cancellation/deadline propagation,
  ownership-vs-scheduler interaction. Gates Cathedral's scheduler chapter.
- [ ] **Atomics + memory model.** Absent entirely. Shape decision (intrinsics
  vs boundary operators vs core library) + which orderings. Gates IPC rings,
  `spawn`, SMP anything.
- [ ] **Separate compilation / component artifact model.** Whole-program
  compiler, one image, absolute frame offsets, fused dispatch loop —
  Cathedral wants independently compiled/signed/hot-swapped components.
  Full backend revamp; meanwhile, codegen decisions keep deepening the
  whole-program assumption (see wiki/architecture/whole_program_assumptions.md
  for which layers are ALLOWED to assume it).
- [ ] **Freestanding target + hardware vocabulary.** No-host-bindings target,
  custom entry, linker/section/physical-address control, volatile/MMIO
  semantics, inline asm beyond `asm { jmp state(...) }` (CR3/MSR/port-IO
  contracts).
- [ ] **Comptime (const eval + trait generators).** Effect-free machines in
  constant positions; `default machine` bodies with `Self::fields` member
  reflection expanded per conformance. Direction frozen (no macros, no #run);
  implementation is a large interpreter+expansion arc. Equatable/Hashable
  synthesis becomes ordinary once this lands.
- [ ] **Generics completion.** Pending canaries exist (generic data
  instantiation, machine-call monomorphization, type params in states);
  const-parameter instantiation/substitution, layout for symbolic lengths.
  Decision-13 bounds are checked on type-reference instantiations; extend
  the check to machine-call monomorphization arguments when those land.
- [ ] **Allocator story.** `Vec` has no runtime; `alloc` is an effect name
  only. Decide explicit allocator/arena capabilities vs ambient heap BEFORE
  implementing Vec lowering.
- [ ] **Repr control for hardware structures.** packed, explicit
  offsets/alignment, untagged unions (page tables, descriptor tables, device
  registers). Chapter 19 has `repr native` only.
- [ ] **Proof engine arcs.** L7 LANDED 2026-06-12: induction via recursive
  contracts + decreases for single-state machines whose body is a chain of
  guarded value/tail-self-call transitions (`proofs/proof_inductive_gauss_sum`
  proves; `inductive_gauss_sum_false_twin` and `..._step_false_twin` reject).
  The recursive arm assumes the machine's own ensures for the call's
  arguments only after the engine discharges a strict decrease of the
  declared measure at that exact call site. Still open: exit-ensures
  anchoring for general bodies (statement-position recursion gets no
  hypothesis — the termination graph does not see those calls), non-tail
  value recursion (compound arm expressions do not parse), quantifiers,
  Bag/Seq lowering, growing the Lean ladder past L7.
- [ ] **Hot-swap semantics.** Quiescence proofs, borrows as swap
  back-pressure, multi-version concurrency mode, replacement declarations
  (`replaces`/`migrates`) — versioned data stage 3+, depends on the
  concurrency model.
- [ ] **Wire encoding families + negotiation.** Beyond stage-2 encoders:
  fixed-width/text families, canonicalization, unknown-field preservation
  policy surface, version negotiation.
- [ ] **Serialized capabilities.** Attenuation + revocability across
  IPC/reboot/network (Cathedral's #1 flagged gap). Depends on wire + the
  capability runtime story.
- [x] **aarch64 runtime convergence.** Resolved: the dungeon hot-potato was
  the encoder using interrupt-clobbered x18 as a scratch register (see the
  backend-residue entry above for the full diagnosis). The scripted dungeon
  loop and the dungeon differential oracle are green on the arm64 host; the
  last interpreter/native divergence (R05/R06 descriptions) was the
  side-room carve guard's lost call-result write, since resolved (see the
  backend-residue list) — the scripted tour is now byte-identical.
- [ ] **Text/string proof domains.** `String::Utf8`/`NoNul` as
  boundary-established carried facts without a byte-level proof tax (frozen
  direction in decision 5; the domains themselves unbuilt).

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
   (record / sum / MIXED; sum-only shipped first, mixed landed 2026-06-11
   -- see the mixed-shapes entry under Outstanding for the recorded
   layout/construction/access rules). First
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
8. **Properties, traits, conformance, and ZII opt-in.** Type PROPERTIES are
   lowercase facts in brackets on the data declaration
   (`data Point [copy, zero_init]`, reusing invariant-parameter syntax);
   acquisition is computed (`sized`) / declared+verified / boundary-asserted —
   no inference, no negative form, not declarable on foreign types. TRAITS
   stay behavior: implemented by ordinary machines (structural satisfaction),
   claimed whole by a standalone conformance item `Point satisfies Equatable;`
   (checks written members, instantiates trait `default machine` bodies,
   synthesizes the CLOSED core derivable set — Slice::index pattern; nothing
   trait-shaped on data declarations). Equality is trait-resolved core
   `Equatable` with synthesized structural `equals`; interim: `==` on
   payload-bearing case values is a compile error (payload-less sums keep the
   tag compare). ZII splits: zero-validity is the unconditional compiler
   guarantee; zero-means-empty is the opt-in `[zero_init]` property which
   owns the zero-case-payload-free rule (the current hard error demotes into
   its verification when properties land). NO macro system ever; user
   structural synthesis, if needed, goes through compile-time execution +
   member reflection (direction only). Case construction stays the brace
   form. See chapters 1, 7, 13, 19 + appendix.
9. **Strict result use.** Discarding a non-unit return value is a compile
   error; intentional discards are spelled `_ = call();`. No per-type
   must_use marker. (Landed 2026-06-10.)
10. **Wire eras.** Generated wire encodings carry one era discriminator
    varint per top-level message/record (era 0 = the pre-versioning body);
    cross-era field-number recycling is legal; cross-era type changes are
    "requires migration" report verdicts, not errors (within-era violations
    and declared-history contradictions stay hard errors); unknown-case-tag
    handling is a wire decode policy (reject / preserve / decode as zero
    case). In-language exhaustiveness is never weakened; `[open]` is
    permanently dropped. See chapter 20 + appendix.
11. **Equality vs membership.** `==` is always value equality, resolved
    through core `Equatable`; `in` is always domain membership (the tag
    test for case domains, value-position legal: `let b: bool = cmd in
    Command::Quit | Command::None;`). A bare PAYLOAD-BEARING case name
    denotes no value — only its domain — so `x == Command::Move` is an
    error suggesting `in`; the brace form `x == Command::Move { dx: 1,
    dy: 2 }` is a constructed value and compares structurally. Equatable is
    IMPLICIT for primitives and payload-less sums (tag identity is
    unambiguous; match desugaring depends on it) and DECLARED
    (`Type satisfies Equatable;`, synthesizing structural `equals` from
    members) for records and payload-bearing sums — deliberately looser
    than Rust's universal derive, since whole-program compilation removes
    the accidental-API pressure. Boundary consequence: adding a payload
    case to a payload-less sum flips it implicit -> declared, erroring
    every `==` site until the one-line conformance is written —
    re-affirming equality after its meaning changed. Tag-clamped guard
    equality is retired as user-visible semantics (it survives only as the
    internal lowering of `in`).
12. **Discard admits effects; pure discards are dead code.** `_ =` accepts
    any CALL today and, by rule, any effectful evaluation later (effectful
    boundary operators, volatile/MMIO reads) — the gate is "evaluation has
    effects", not "is a call". Discarding a provably pure call (resolved
    callee has an empty effect set AND no `&mut`/out parameters — both
    signature-level facts) is a hard error, not a warning. Discarding a
    pure non-call expression stays a parse error. (Landed 2026-06-11:
    purity is judged against the callee's INFERRED transitive effect
    surface, not the declared list alone, so an undeclared-effects machine
    that transitively reaches `console.write` never counts as pure.)
13. **Property bounds: brackets attach to what they follow, everywhere.**
    Type parameters take bracket facts inline: `data Box<T [copy]> [copy]`.
    The Rust-style colon bound (`<T: copy>`) and the attribute-prefix form
    (`[copy]` on its own line) are rejected — colon would split the
    spelling system, and a floating prefix line is positional metadata (the
    attribute magic properties deliberately avoid). Leaves
    `T [copy] satisfies Equatable` room for trait bounds without
    collision.
14. **`Versioned<T>` container.** A permanent builtin template type
    `{ era: u32, payload: union-of-eras }` — the only thing version match
    arms are legal on (matching `Counter::v1(...)` on a PLAIN value stays
    an error; ordinary values never carry era tags). Constructed only at
    boundaries (wire decode, storage read, hot-swap edges); consumption is
    ordinary tag dispatch where the paren arm form binds the WHOLE
    historical value (`Counter::v1(old) -> ...`; braces stay field
    binding). `era` is read-only source-queryable. Migration-chain
    completeness is a report verdict, not an error (an arm may handle an
    old era manually). See chapter 21.
15. **Lifetimes: the Rust model, adopted wholesale.** A call's output may
    borrow an input; lifetime parameters (tick spelling, declared in the
    same `<>` list as types and `const`) express which:
    `machine header<'buf>(buffer: &'buf [u8], scratch: &mut [u8]) ->
    &'buf string`. ELISION covers the common cases (one ref input → output
    borrows it; `&self` → self) so most signatures stay annotation-free.
    Borrow-carrying data is IN-MODEL from day one
    (`data ChatMessage<'buf> { body: &'buf string; }`). House style:
    descriptive lifetime names (`'buf`, `'arena`), not `'a`. Rejected
    spellings: `from <arg>` clauses, `borrows` clauses, keyword
    region/origin parameters, Mojo-style bracket origins (collide with
    slice/property/invariant brackets). Unblocks zero-copy wire decode and
    view-returning machines. See chapter 2 + appendix.

## Next Up (highest leverage)

**Landed 2026-06-11 (proof soundness: call-requires collection for free
machines + boundary traits).** Two call shapes silently never produced
call-site `requires` obligations, so callee contracts passed vacuously
(negative-control probes compiled clean): (a) FREE top-level machine calls --
the receiverless target stayed an invalid symbol through the frontend (the
backend dispatched by name), so borrow/contract collection saw nothing; in
STATEMENT position the call did not even resolve (`machine X has no local
state` from validation). (b) BOUNDARY-TRAIT machine calls
(`self.console.show(item)`) -- the trait machine signature was invisible to
`resolve_state_call_target`, and signature-owned contract facts were
explicitly excluded from call-fact matching. Fixes: symbol resolution now
points receiverless free-machine calls at the machine's entry state
(builtins still win); validation accepts the free-machine statement call
(strict result use applies, named as spelled); the checked-trees call-target
resolver accepts cross-machine state symbols and trait machine signature
symbols; `contract_target_from_state_symbol` maps trait signatures to their
owning trait; `append_contract_fact_refs` matches StateSignature owners; the
instantiation path (`call_target_parameters`) reads parameters from machine
states OR trait signatures, so callee-parameter -> caller-argument place
mapping and caller-requires discharge work for both shapes, and mutation
invalidation strikes the instantiated facts (probe verified: interleaved
`item.value = 0` before the call rejects with the invalidation detail).
Chapter 18 authority flow is now load-bearing: `Filesystem::write_bytes
requires folder in Folder::Writable` is enforced at the caller. Corpus
fallout: NONE (suite stayed at baseline; host `capability` blocks are
dropped at lowering and never reach contract collection). New canaries: fail
`domains/call_requires_free_machine_value_unproven`,
`domains/call_requires_free_machine_statement_unproven`,
`domains/call_requires_boundary_trait_unproven`; pass
`domains/call_requires_free_machine_satisfied_by_caller_requires`,
`domains/call_requires_boundary_trait_satisfied_by_caller_requires`.
Residue: PLATFORM state-signature `requires` (calls through platform-typed
contained objects) are still never collected -- same vacuity, third shape,
needs the same treatment when platforms matter; `capability { entry ...
requires }` blocks are dropped at symbol-resolution lowering, so host
capability contracts (omega/host/contracts) remain unenforced until
capabilities lower at all.

**Landed 2026-06-11 (decision 12 implementation).** Pure discards are now dead
code: `_ = call();` rejects when the resolved callee's inferred TRANSITIVE
effect set is empty AND its signature takes no `&mut` out-parameters
(`validate_effect_plan` owns the check; the transitive surface — not the
declared list — is the purity source, so a no-declaration machine that
transitively reaches `console.write` stays discardable). New canaries:
`fail/calls/pure_discard_dead_code` and
`pass/calls/effectless_mut_out_param_discard_compile` (&mut out-param, no
effects — must stay legal); `runtime_explicit_discard_executes_exit` is
unaffected (its callee writes through `&mut Tally`).

**Wave landed 2026-06-10 (decisions 8/9/10 implementation + backend gaps).**
Six lanes merged, suite 179/179, differential oracle fully matched:
(a) type properties `data Point [copy, zero_init, send]` parse + verify
(copy/send structural, zero_init owns zero-means-empty incl. the DEMOTED
zero-case rule); (b) standalone conformance items `Point satisfies
Equatable;` validate against written attached machines (default
instantiation/core synthesis still pending -- the comptime direction);
(c) interim `==` error on payload-bearing cases in statement position;
(d) strict result use: discarding a non-unit call result errors, `_ =
call();` is the explicit discard (only ONE corpus file needed the sweep);
(e) wire era chain checks + migration verdicts + legal recycling;
(f) versioned data stage 1 (historical-shape symbols, `Counter::v1` types,
migration-machine spelling compiles natively); (g) case PAYLOADS lower
natively (tag-prefix writes, payload member reads, tag-only guard compares;
pending canary promoted, ACTIVE_PENDING_CANARIES empty); (h) value-position
calls to FREE stateful machines dispatch and deliver values (incl. looping/
recursive shapes). Known interim semantics flagged for design review:
`_ =` accepts only calls. (Tag-only case equality in guards was RESOLVED by
the decision-11 landing below: the tag clamp is no longer user-visible
equality semantics, only the internal lowering of `in`.)

**Decision 11 landed 2026-06-11 (equality vs membership).** `in` now accepts
implicit case domains at use sites: `cmd in Command::Move` (payload-bearing
included) and unions `cmd in Command::Quit | Command::Move` work in value
position and as transition guard subjects, lowering to tag-equality compares
in the resolved->typed stage. Transition case arms desugar to MEMBERSHIP at
parse time (not `==`), so the bare-payload-case `==` check runs on the
RESOLVED trees and covers every position -- statements, guard
subjects/conditions, transition target arguments, domain `when` classifiers
and proof facts, machine contracts -- with a message suggesting `in`; the
brace form keeps the structural-equality interim error, payload-less `==`
stays legal everywhere. The guard tag clamp survives only as the internal
lowering of `in` (and payload-less `==`); the runtime-value expression paths
gained the same tag clamp for case compares inside boolean trees. New
canaries: pass+RUN `data/case_membership_value_exit`,
`data/case_membership_union_guard_exit` (both in the differential oracle);
fail `data/bare_payload_case_equality_suggests_in`,
`data/bare_payload_case_equality_guard`.

**Decision 13 landed 2026-06-11 (property bounds on type parameters).**
`data Box<T [copy]> [copy] { value: T; }` parses everywhere
`parse_type_parameters` runs (data, machine, trait, operator); the bracket
fact list is the SAME parse as the data-declaration property list (closed
set, duplicates/`sized`/unknown rejected). `zero_init` is accepted as a
bound: its structural rule reads fields, so it is checkable at
instantiation exactly like copy/send. The Rust-style colon bound
(`<T: copy>`) and the attribute-prefix form (`<[copy] T>`) are rejected
with the bracket spelling suggested. The structural copy/send/zero_init
verifier now accepts a field whose type parameter declares the matching
bound (and suggests `T [copy]` when it does not), and every VALIDATED
type-reference surface (data fields, domain targets, machine owned data,
state locals, state parameters/returns) checks instantiation arguments
against the base data's parameter bounds — in-scope bounded parameters
count as carrying their bound. An instantiated generic whose base declares
a property now also satisfies the structural walk (`Box<i32>` is copy
inside another `[copy]` data). NOT yet checked: machine-call
monomorphization arguments (generics completion arc). Canaries:
`pass/generics/property_bound_type_parameter`,
`fail/generics/{property_bound_missing_on_field,
property_bound_violated_at_instantiation, colon_bound_rejected}`.

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
`match` and version migration matching. Full canary suite is green locally
(`cargo test -p omega-compiler --test canary_suite`, 163 Rust tests); pass/fail
canary counts can change without changing the Rust harness test count because
many canaries are batched. The proofs false twins were promoted to
`canaries/fail/proofs/` when the contract entailment engine landed (empty-body
proof machines now PROVE or REJECT in-language contracts); see
`wiki/proof_engine_roadmap.md`.

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

**Data version semantics follow-up.** STAGE 1 DONE (2026-06-10): each
`version vN { ... }` block now lowers to a real historical-shape data
definition `Data::vN` with root symbols and member resolution, so
`Counter::v1` is a nameable type usable in machine signatures and generic
arguments; the chapter-21 migration spelling
`machine Counter::from_v1(old: Counter::v1, out: &mut Counter)` compiles
end-to-end including native lowering, and version-scoped machine paths
(`Counter::increment::v1`) type-check `self` against the v1 field set.
Declared-history contradictions (duplicate/non-canonical/nested version
names, version-scoped machines targeting undeclared versions) are compile
errors. STAGE 2 DONE (2026-06-11): historical-shape VALUES construct —
`Counter::v1 { counter: 3 }` resolves the brace literal to the version
block's shape definition (NOT a case of `Counter`; constructing an
undeclared version is a compile error), struct-literal field names now
validate against the constructed shape's declared members (current shape,
historical shape, and case-payload literals alike), and a call through the
data TYPE name (`Counter::from_v1(old, &mut current)`) resolves to the
attached machine, so the chapter-21 migration runs end-to-end — the first
runtime migration canary (`versioning/runtime_version_migration_exit`,
exit 70) passes natively AND in the differential oracle. Version MATCH arms
(`Counter::v1(old) ->`) got their stage-2 ruling: values carry no era tag,
so every value has the current shape and a version arm can never be
selected — the arm is rejected as UNREACHABLE (fail canary
`versioning/match_on_version` pins the diagnostic) rather than lowered with
fake runtime semantics. STAGE 3 frontier: the era tag itself (and decision
10's wire-era ride), era-tagged containers that make version matching
selectable, migration chains / `replaces` / quiescence obligations.

**Wire data semantics follow-up.** Stage 1 (validation + compatibility) is
done: wire schemas now lower through symbol-resolved and typed trees as their
own root family (`WireSchema` with arena-stored members and a `WireSchema`
symbol kind), `omega-validation` rejects duplicate/reserved tag misuse,
duplicate versions, unresolved field types, and version-vs-current type
changes or unreserved retirements (fail canaries under `canaries/fail/wire/`),
and every compile emits a `04_wire_protocols.txt` compatibility report with
per-version verdicts. DECISION 10 LANDED (2026-06-10): the checker and the
report now walk the version chain `[v1, v2, ..., current]` comparing only
ADJACENT eras; cross-era type changes are "requires migration" report
verdicts (compile clean); retiring a documented number without reserving it
is era-scoped to the successor and stays a hard error; cross-era
field-number recycling is legal (per-scope `reserved`); pass canaries cover
recycling + type-change migration verdicts. STAGE 2a LANDED (2026-06-11):
era assignment (era 0 = the pre-versioning body; version blocks count up in
declaration order; the current body is the highest era, reported per schema),
the compiler-recognized `Schema::encode_wire(&value, &mut out, &mut written)`
call (validated front-end: stage 2a scalar set, field coverage by name+type,
worst-case out-buffer capacity so the emitted code needs no runtime bounds
checks), and compact_binary v0 framing emitted through two new wire-append
operations (literal framing byte + runtime scalar varint) implemented on both
ISAs with widths/relocation-offset functions asserted against the encoders.
STAGE 2b (current-era decoder) and STRING-FIELD ENCODE landed 2026-06-11
(see the wire stage 2 bullet above for the String storage decision and its
known holes). Still needed: String decode (borrow-facts follow-up),
nested/repeated fields, wire-schemas-as-program-types, runtime layout of
wire values, encoding-family semantics beyond compact_binary v0, and version
negotiation.

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

## Vertical Slices

### Capabilities And Authority

- [x] Capability facts flow through returns/derives/acquires across nested calls,
  not just direct boundary calls: `build_capability_facts` runs a call-graph
  fixpoint that folds a callee's verb into its caller when the authority value
  reaches the caller (capability-typed return for `acquires`/`returns`,
  capability return or parameter for `derives`). Propagated facts carry the
  helper state as provenance (`CapabilityFlowFact.via_state_symbol`) and the
  boundary blast radius renders it (`Backup::stage acquires via Vault::pick`).
  Canaries: `capabilities/acquires_through_helper_return` (two-level acquire
  chain), `capabilities/derives_through_helper`.

### Core Boundary Primitive Registry

- [x] Populate `BoundaryProvider.contract_ref`/`effect_set`/`target_applicability`
  from the bound operator instead of empty defaults. The populated registry is
  surfaced in the boundary report artifact (`10_boundary.html`, "Boundary
  Providers" section): per provider, the governing contract, authority effects,
  target applicability, and origin package.

### Proof-Backed Indexing And Subslicing

- [x] Thread refined subslice diagnostics through operator-contract errors once
  `Slice::from/to/range` contracts drive checking directly. RESOLVED
  (2026-06-12): a failed operator-sourced bound now names the spelled operator
  and its contract for browsability — e.g. ``cannot prove `start <= end && end
  <= items.len` — the `requires` of `Slice::range` (spelled `[..]`)`` appended
  to the fact-level failure. Attribution only fires when the core slice surface
  is imported (the obligation is operator-sourced); literal-shape diagnostics
  stand alone otherwise. Pinned by
  `fail/slices/subslice_range_operator_contract_unproven` and
  `fail/slices/index_operator_contract_unproven`.
- [x] Represent length facts and window-shrinking facts as first-class slice
  proof vocabulary (non-empty already exists). LANDED (2026-06-12): the
  vocabulary is `minimum_lengths` (floor), `exact_lengths` (pinned), and
  `window_parents` (carve relation) in `RangeFacts`. New derivations: a
  start-only tail `items[a..]` with constant `a` shrinks the parent's floor and
  exact length by `a` (`prove_shrunk_window_length`), and a constant-bounded
  range over a symbolic-length base discharges its `start <= end` ordering by
  folding both bounds. Consumers: index proofs over the derived window length
  (`pass/slices/window_shrink_min_length_tail_index_compile`,
  `pass/slices/window_literal_bounds_min_length_parent_index_compile`; one-past
  rejections pinned in the matching `fail/slices/*_unproven` canaries).
  Soundness companion: reassigning a local collection now forgets its
  label-keyed facts (floors, exact lengths, window relation, position proofs)
  via `forget_collection_facts` — a stale floor from the old value must not
  prove indexes into the new one
  (`fail/slices/window_reassigned_shrunk_floor_unproven`).
  Honest scope: symbolic (non-folded) bounds — e.g. `tail[parent_len - 3]` —
  still need a symbolic length algebra; only constant-folded offsets shrink.
- [x] Ensure alias and borrow facts understand subslice overlap conservatively.
  VERIFIED CONSERVATIVE (2026-06-12): two `&mut` windows of the same base are
  rejected unless their literal bounds prove disjointness
  (`windows_may_overlap` defaults to overlap on any unknown bound; the borrow
  pass reuses it via the loan-overlap engine). Probes: `items[0..2]`+`items[2..4]`
  accepted, `items[0..2]`+`items[1..3]` rejected, `items[0..2]`+`items[..]`
  rejected. Pinned by `pass/slices/disjoint_mut_subslice_windows_compile`,
  `fail/slices/overlapping_mut_subslice_windows_rejected`, and
  `fail/slices/unknown_bounds_mut_subslice_windows_rejected`.

### Slice Runtime Descriptor Semantics

- [x] Blank-room rendering RESOLVED (verified 2026-06-11): native dungeon
  room lookup/render now produces labels/descriptions byte-identical to the
  interpreter on the canonical scripted loop (the x18 reserved-register fix
  closed the remaining corruption). The final dungeon divergence (R05/R06
  data-driven descriptions) was the side-room carve guard's lost call-result
  write, since resolved in the backend-residue list — descriptor
  initialization itself was already fixed.
- [x] Generalize subslice descriptor pointer offsets beyond fixed-array alias
  copy special cases. DONE (2026-06-12): every slice-descriptor write consumer
  (locals, transition arguments, branch preludes, mutations) routes through one
  seam — `emit_runtime_frame_slot_slice_descriptor_write_in_table` now tries the
  generalized runtime-descriptor subslice after the literal fixed-array path.
  Newly lowering shapes (all interpreter-differential-verified):
  subslice-of-param into a LOCAL (`let tail = sub[1..]`, previously a silent
  whole-descriptor copy), nested subslice in one expression (`sub[1..][1..]`,
  literal layers fold into a window bias; previously a silent un-offset
  descriptor natively AND an interpreter reject — `eval_subslice` now evaluates
  nested range-indexed bases as views), and runtime-start over a subslice local
  (`tail[start..]`, bias rides the indexed-address op's field offset).
- [x] Generalize start-only/end-only/bounded descriptors beyond literal
  fixed-array-backed views. DONE (2026-06-12): bounded (`sub[1..4]`) and
  end-only (`sub[..2]`) literal ranges over runtime descriptors already lowered
  (now pinned by canaries); RUNTIME bounds are new — `sub[start..]` computes
  ptr via `WriteRuntimeFrameIndexedAddressToRuntimeFrame` (its aarch64 width
  table was stale by 40 bytes — fixed to use `runtime_frame_index_setup_width`)
  and len as a storage-storage subtraction; `sub[..end]` reads the runtime
  length from `end`'s slot; literal inclusive ends (`sub[1..=3]`) fold to
  `end + 1` at selection time. STILL UNSUPPORTED (loud, see below): computed
  bounds (`sub[offset + 1..]`), RUNTIME inclusive ends (`sub[..=n]`, needs a
  +1 at runtime), and runtime bounds in a NESTED inner layer. Slice-typed
  `data` fields (`items: &[T]`) do not parse, so the "machine-field slice"
  base shape is not expressible at the language level today.
- [x] Add focused pass/fail canaries for each newly supported subslice descriptor
  lowering shape. DONE (2026-06-12): eight new runtime canaries (suite + RUN +
  differential): `runtime_subslice_param_bounded_range_exit`,
  `runtime_subslice_param_end_only_exit`, `runtime_subslice_param_local_exit`,
  `runtime_subslice_runtime_start_exit`, `runtime_subslice_runtime_end_exit`,
  `runtime_subslice_nested_of_param_exit`,
  `runtime_subslice_runtime_start_over_local_exit`,
  `runtime_subslice_param_inclusive_end_exit`.
- [x] Unsupported subslice shapes now fail LOUDLY instead of silently keeping a
  stale/garbage descriptor: the `descriptor_argument_blockers` emission pass
  verifies every range-indexed transition argument writes its callee parameter
  slot and every subslice-initialized slice local writes its descriptor slot,
  and blocks emission naming the state, statement, and expression otherwise
  (probed with `sub[offset + 1..]` in both argument and local position — both
  previously compiled and exited wrong; both now block).
- [x] Keep backend reports explicit about descriptor construction and mutation.
  Verified 2026-06-12 by probe: each construction renders one line per half —
  `write runtime-frame pointer @T = &(runtime_frame@desc[runtime_frame@idx * elem]) +bias`
  for the pointer and a `write runtime storage binary … Subtract …` /
  `write runtime storage integer …` for the length — base, start source, and
  length source are all readable. No gaps found; nothing changed.

### Measures, Orderings, And Rankings

- [x] Support builtin/default inference for plain `decreases value` only when
  unambiguous. DONE (2026-06-11; core inference had landed earlier as
  "Infer default decreases order"). The rule: plain `decreases value` infers a
  builtin ranking only when the value's type makes it unambiguous — unsigned
  integer kinds (`usize`, `u8`-`u64`, `nat`, and `slice.len` members) get
  descending naturals; slice-typed values get `Slice::Length`; `upper - lower`
  is the named bounded distance. Everything else (signed integers, floats,
  structs) errors with a type-aware diagnostic naming the value and the reason
  (e.g. "cannot infer a ranking for `decreases remaining` ...: signed values
  have no default well-founded order -- select one with
  `decreases remaining -> View`"). RULING: a declared `measure` is NEVER
  selected implicitly, even when it is the only one declared for the value's
  type — only true builtins infer, so declaring a second measure later cannot
  silently change or break distant `decreases` clauses at a distance. Matching
  declared measures are suggested by name in the diagnostic instead
  (fail canary `termination/default_order_declared_measure_not_inferred`
  locks the ruling; pass canary
  `termination/default_order_unsigned_width_countdown_compile` covers
  non-`usize` unsigned widths).
- [x] Replace arithmetic-facing proof UX such as `limit - index` with named
  bounded-distance rankings. DONE (2026-06-12). The named view is
  `Nat::BoundedDistance` ("rank by the natural-number distance from the lower
  value up to the upper bound"), following the existing `Nat::Descending` /
  `Slice::Length` Type::Name pattern, which the view position already parses
  with no grammar change. What landed: (a) plain `decreases upper - lower`
  resolves to the distinct `RankingOrder::BoundedDistance` (no longer folded
  into NatDescending), so diagnostics and the checker name the ranking;
  (b) explicit selection `decreases limit - index -> Nat::BoundedDistance`
  (pass canary `termination/bounded_distance_named_view`); (c) the inverted
  spelling `decreases index - limit` is recognized — the checker probes the
  swapped operands, and when they prove, rejects with a diagnostic that names
  the right shape ("... inverts the named bounded distance --
  `Nat::BoundedDistance` ranks `upper - lower` ... write
  `decreases limit - index`"; fail canary
  `termination/bounded_distance_inverted`); (d) the L7 induction gate also
  accepts the named view — the distance polynomial goes through the identical
  strict-decrease + non-negativity check (pass canary
  `proofs/proof_inductive_climbing_sum`, step-false twin
  `proofs/inductive_climbing_sum_step_false_twin` pins that the hypothesis
  actually enters through this gate); (e) the ambiguity diagnostic's browsable
  builtin-view list now includes `Nat::BoundedDistance`. DECIDED 2026-06-12
  (maintainer): the use-site subtraction is NOT acceptable permanent
  surface — build the argumented view spelling
  `decreases (index, limit) -> Nat::BoundedDistance` (tuple form; the
  arrow's left side stays uniformly the ranked subjects) and retire
  `decreases limit - index` once it lands. Grammar-surgery scope: the
  ranking-view position is a plain identifier path
  (`parse_path_handle_span` in
  `omega-tokens-to-syntax-trees/src/parser/machine/clauses.rs`) and
  `decrease_order` is `HandleSpan<Identifier>` through all three tree
  representations, so view arguments need new syntax, storage, and symbol
  resolution. NOTE (pre-existing bug,
  RESOLVED 2026-06-12 below): a `requires` clause on a recursive machine used
  to overflow the compile-time contract evaluator's stack
  (`ContractExpressionEvaluator::integer_value` followed the self-call site's
  arguments in a loop), which is why `proof_inductive_climbing_sum` states its
  theorem as `result >= acc + limit - index` (true without a precondition)
  instead of the equality that would need `requires index <= limit` (the
  climbing canary's weaker theorem statement is kept as-is).
- Resolved 2026-06-12: `requires` on a recursive machine no longer crashes
  the compiler. Root cause: the contract evaluator's constant walk
  (`checks/contracts/evaluator/` in omega-typed-trees-to-checked-trees)
  resolves a callee parameter to the call-site argument expression to
  discharge `requires` by constant propagation; at a SELF call site the
  argument mentions the same parameter (`n` resolves to `n - 1`, whose `n`
  resolves to `n - 1` again), so `integer_value`/`resolved_expression`
  alternated forever. The pre-existing same-handle check in the Name arm only
  caught cycles of length 1. Fix: two active-expression stacks
  (`active_evaluations`, `active_resolutions` on
  `ContractExpressionEvaluator`, threaded through `guarding_cycles`) detect
  re-entry into an expression still being evaluated/resolved and STAND DOWN
  with None -- unknown never proves and never falsely rejects, so discharge
  falls through to the semantic provers (arm facts, caller requires).
  Legitimate constant following is untouched (pass
  constraints/scalar_requires_satisfied_by_literal and the rest of the suite
  are unchanged). Regression pin: pass canary
  proofs/recursive_machine_with_requires_compiles -- a recursive gauss_sum
  threading an untouched `limit` parameter with `requires limit > 0`,
  discharged by the literal at the outer call (constant walk) and by the
  caller's own requires at the recursive call; its value is that it compiles
  AT ALL. Unprovable shapes on recursive machines (e.g. `requires n >= 0`)
  now produce the normal cannot-prove diagnostic instead of a stack overflow.
- Resolved 2026-06-11: shrinking-slice recursion runtime exit canary added as
  `termination/runtime_shrinking_slice_recursion_exit` (suite ACTIVE list +
  dedicated run test + differential RUN_CANARIES; the parked
  `canaries/run/shrinking_slice_recursion_total_probe` is deleted). Root cause
  of the wrong native total: `resolve_runtime_storage_place_in_table`'s
  path-based fall-through DROPPED a root element index over a slice-descriptor
  frame slot, so a threaded `items[0].value` transition argument resolved to a
  plain place over the descriptor slot itself — `take` received the data
  pointer's low bytes (observed exit 152 = (4*ptr + 4+8+12) & 0xff; 152 is not
  a multiple of 5 while every element is, the fingerprint that ruled out any
  element-sum). Fixed in instruction selection: the resolver now refuses an
  unhonorable root index (descriptor slots always; inline fixed arrays for
  index != 0), transition-argument materialization gained a descriptor-aware
  `CopyRuntimeFrameFixedIndexedToRuntimeFrame` strategy, and
  `argument_source_frame_range` reports the descriptor slot as the read range
  so the same-context overlap staging (source -> scratch -> target) still
  triggers — without it the in-place `items[1..]` update would shrink the
  window BEFORE the head read. The statement-position shape of the same
  accumulation still over-executes natively — that is the separate non-guard
  executor-of-record residue, not this argument-lowering bug.

### Operators And Domains

- Consolidated 2026-06-11: the two parallel operator-resolution surfaces are
  now one authority. `omega_typed_trees::operator::resolve_spelling` (spelling
  -> root + domain-owned candidates, receiver-type narrowing) is the single
  use-site resolution implementation — resolution is a typing-stage decision
  per the pipeline Ownership Rule — and the checked stage
  (`omega-typed-trees-to-checked-trees/src/operators.rs`) only records its
  outcome as durable evidence (`CheckedOperatorFacts`, candidate contract
  spans, `ProofFacts.contract_operator_uses`) instead of re-resolving. The old
  operand-key `resolve_spelling`/`SpellingDispatch` had no callers and was
  deleted, and `omega-validation` dropped its private copy of the operand
  signature normalizer in favor of the typed-trees one. Declaration-conflict
  diagnostics (duplicate spellings, competing domain meanings in
  `omega-validation`) and use-site resolution evidence (checked facts) answer
  different questions and intentionally remain separate consumers of the one
  authority; the bounds-from-`requires` seam keeps consuming the typed-trees
  helpers unchanged.
- Resolved 2026-06-12: positive proof-context operator selection landed — only
  facts in the CURRENT context can select a domain-operator meaning. Spelled
  binary uses are now recorded as operator evidence (`build_operator_facts`
  gains a `Binary` arm; builtin-only arithmetic with no spelled candidates
  stays unrecorded and untouched), and a post-flow pass
  (`operators/selection.rs`, run from `build_check_facts` after flow facts
  exist) admits a domain-owned candidate only when the LEFT operand's domain
  membership is PROVEN by the semantic contexts entering the statement — the
  same invalidation-adjusted contexts the call-`requires` discharge reads, so
  caller `requires`, call `ensures`, and interleaved-mutation invalidation all
  participate. Selection ruling from chapter 8's "participates only if it
  exposes a unique operator meaning" text: exactly ONE admissible (proven)
  domain meaning wins the expression over the builtin — the `requires`
  deliberately narrowed the context; ZERO admissible domain meanings leave the
  ordinary meaning in place when one exists (unique root spelled candidate, or
  the builtin scalar operation for primitive operands → evidence status
  `BuiltinFallback`) and reject otherwise (`Inadmissible`, the positive-proof
  error); TWO or more admissible domain meanings are ambiguous (largely
  precluded by the declaration-level competing-meanings rejection). What
  selection PRODUCES: evidence (`CheckedOperatorFacts` records the winning
  meaning; `selected_candidate` exposes it) — domain operators have no bodies,
  so a selected meaning never changes lowering (no hidden runtime tag, per the
  chapter), and as an honesty guard a selected binary-spelling meaning that
  carries `requires` contracts is rejected loudly because contract discharge at
  spelled binary use sites is not wired yet (slice `[]`/`[..]` discharge
  through the ranges seam is unaffected). Canaries:
  pass `domains/domain_operator_proven_fact_selects_meaning` (+ suite test
  asserting the domain meaning is the recorded selection),
  pass `domains/domain_operator_unproven_keeps_builtin_meaning` (+ suite test
  asserting `BuiltinFallback`), fail `domains/domain_operator_meaning_unproven`,
  fail `domains/domain_operator_meaning_invalidated_by_mutation`, and the
  previously-unregistered `domains/domain_operator_spelling_selected` (pass)
  and `domains/domain_operator_competing_spelling_meanings` (fail) now run in
  the sweeps.
- Resolved 2026-06-12: `requires` contracts of selected spelled BINARY operator
  meanings now discharge at the use site (`checks/operators/requires.rs`). The
  selected candidate's contract span — preserved in the operator evidence
  precisely for this — yields the `requires` proof facts, each instantiated
  over the actual operands (parameter -> operand positional mapping at `Name`
  nodes, the call-`requires` label-instantiation precedent from
  `checks/contracts/labels/calls.rs`; operators have no `self` and no `result`
  binder) and proven against the semantic contexts entering the use's
  statement — the same invalidation-adjusted contexts the selection pass and
  the call-`requires` discharge read. Membership clauses prove via
  `domain_implies` + place/value label match; boolean clauses decompose
  And/Or like the call prover and accept direct boolean facts or
  domain-membership-derived facts (`domain_proves_expression_label`).
  Unproven clauses report the indexed seam's contract-naming attribution
  shape: ``cannot prove `b in Quantity::Additive` — the `requires` of
  `Quantity::Additive::add` (spelled `+`)``. The honesty guard that rejected
  contract-carrying binary selections outright is retired — selections are
  now checked, not refused. Slice `[]`/`[..]` uses keep discharging through
  the ranges seam, unchanged. Canaries:
  pass `domains/domain_operator_requires_discharged` (caller facts prove both
  the selecting membership and the operator's `requires`),
  fail `domains/domain_operator_requires_unproven` (same shape minus the
  `right`-operand fact, asserting the attribution diagnostic).

### Ownership, Borrowing, And Views

- [ ] Continue appending ownership transfer/drop events from the remaining
  value-expression sites. (Now covered: operator-result + let-init seams,
  assignment-target owned production, statement-level operator/boundary calls,
  terminal/bare expression statements, and exit-drop obligations for owned
  by-value state parameters. Operator argument/receiver policies resolve by
  spelled path — call sites carry no operator symbols today — and a static
  type-name receiver like `String::with_capacity` no longer records a bogus
  type-symbol move. `self.field` event roots re-root at the machine symbol so
  downstream stages, which filter `self` parameters, can still resolve them.
  Remaining: move-subtraction/liveness so exit drops become per-edge truths
  instead of conservative obligations, and events for owned operator results
  produced directly in argument/transition-value positions, which have no
  place to root at yet.)
- [ ] Lower abstract ownership summaries into explicit backend transfer and
  cleanup operations. (First landing: the encoded ownership summary now
  renders per event in the backend report's Artifact Semantic Spine — place,
  machine/state, and source point — proving the events survive checked trees
  through the encoded machine. Real transfer/cleanup operations are
  deliberately NOT emitted yet: no type carries a cleanup machine, so every
  drop is semantically empty and emitting no-op cleanup code would be dead
  weight. Revisit when drop-bearing types land — Vec/String real storage and
  the allocator story.)

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
- Current local suite status (2026-06-11, macOS ARM64 host): `cargo test -p
  omega-compiler --test canary_suite` is 184/184 and the differential oracle
  is 5/5, dungeon included — FULLY GREEN. The aarch64 encoder convergence
  wave closed the 30-failure arm64 gap, and the dungeon "hot-potato" root
  cause was the encoder using x18 (the Darwin reserved platform register,
  zeroed by XNU on kernel→user returns) as copy scratch — fixed by register
  substitution, pinned by the interrupt-soak canary under `pass/dungeon/`.
  Full `cargo test --workspace` is also green. No registered pending
  canaries (the proofs false twins were promoted to `fail/proofs/` by the
  entailment engine; see `wiki/proof_engine_roadmap.md`). Keep this line
  current when backend/runtime work moves canaries between `pass`, `fail`,
  and `pending`.

### Docs

- Add a dedicated guide section/chapter for core semantic types once syntax
  stabilizes; add navigable core docs alongside `omega/language/core`.
- Keep traits/modules/host-boundaries sequencing coherent; keep speculative
  topics clearly labeled as working direction.
