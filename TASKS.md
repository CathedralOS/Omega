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
  wrong-era-rejection run canaries in the differential oracle. Remaining:
  historical-era decode via `Versioned<T>` (after the stage 3 sign-off),
  strings/nested/repeated fields, wire-schemas-as-program-types, runtime
  layout of wire values, encoding families beyond compact_binary v0,
  version negotiation.
- [ ] **Versioned data stage 3.** Era tag + the wire integration decision 10
  assumes; era-tagged containers that make version MATCH arms selectable
  (stage 2 ruled them unreachable — no value can hold a historical era yet);
  migration chains, `replaces`, quiescence obligations. (Stage 2 landed
  2026-06-11: historical-shape construction, the type-name migration call,
  the first runtime migration canary, struct-literal field validation.)
  PROPOSED DESIGN AWAITING SIGN-OFF (scouted 2026-06-11): a builtin
  `Versioned<T>` container — `{ era: u32, payload: opaque bytes of the
  era's own shape }`, the trait-object pattern — constructed at boundaries
  only (chapter 21: ordinary values never carry era tags); version match
  arms become legal ONLY on `Versioned<T>` subjects (tag compare + shape
  reinterpretation per arm); the wire decoder is NOT a prerequisite.
  Stage 3b (no new surface, dispatchable independently): migration-chain
  completeness validation along the declared version chain. Open decisions
  for the maintainer: container name/permanence, era field width (u32
  proposed), payload storage (opaque bytes vs union-of-eras), whether
  `era` is source-queryable, and incomplete-chain severity (warning
  proposed). `replaces`/quiescence stay deferred behind the concurrency
  model.
- [ ] **Equatable synthesis / conformance defaults.** EQUATABLE SYNTHESIS
  LANDED (2026-06-11): `Type satisfies Equatable;` on a record or
  payload-bearing sum makes `==`/`!=` legal -- expanded INLINE at
  resolved->typed lowering into field compares (sums: OR over cases, tag
  compares first, then payload fields), riding existing backend/interpreter
  comparison machinery; the interim `==` error is retired for conforming
  types and extended with a declare-the-conformance suggestion for
  non-conforming ones; a written `Type::equals` wins (`==` lowers to a
  call); prerequisites error at the conformance item (every field scalar /
  payload-less sum / conforming; `String` fields rejected -- no native
  value-position text compare; recursive types rejected). The interpreter
  short-circuits `&&`/`||` and ZII-defaults enum fields to the zero case;
  the native value-operand resolver reads oversize enum places as their tag
  prefix in tag compares (was a silent statement drop for two-field
  payloads). Canaries: pass+RUN `traits/equatable_record_equality_exit` +
  `traits/equatable_sum_payload_equality_exit`, fail
  `traits/equatable_missing_conformance_suggested` /
  `equatable_field_not_equatable` / `equatable_recursive_type` /
  `equatable_string_field_unsupported`. STILL OPEN: a CALLABLE synthesized
  `Type::equals` machine (comptime/trait-generator arc), trait `default
  machine` instantiation for other traits, `String`/recursive Equatable
  support, equality in contracts/domain facts (no typing scope there), and
  written-equals signature matching against `&Self` (validation accepts
  `Self` in trait signatures; substitution per conformance is unchecked).
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
  pass data/match_default_satisfies_exhaustiveness. STILL OPEN: MIXED shapes
  (common fields + case part). Payload sums are done; `self in Type::Case`
  and unions at use sites landed with decision 11.

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
- [ ] Dungeon residual, ONE draw: a mis-dispatched `apply_event` ARM during
  R00's carve. Sentinel tracing (counter bumps compiled into each arm state)
  on a generation-trimmed dungeon shows native runs the GOLD-CACHE arm
  (correct, matches interpreter) AND the ENEMY arm chain for the same carve —
  roll_enemy_kind then dispatches with a stale `depth` (R00 is depth 9 but
  the BAT arm `depth <= 1` fires), drawing once. R01 dispatches quiet
  correctly. Emission is NOT duplicated (each draw site appears exactly once,
  in its own dispatch case, per the backend report) — the dispatch STATE
  itself reaches the enemy cases, so this is an arm-selection/flow bug, not
  executor multiplicity. Small probes of the same ladder (4 arms, compound
  NeedsRuntimeExpression range guards, selected middle arm, nested reward
  sub-dispatch, &mut args) all pass natively — the misfire needs deeper
  dungeon context (slice-ref room via room_mut, field-machine chain
  events.enemies.rng, two carve_room call contexts). Repro: trim
  maze_builder::build to R00+R01 (stop after build_main_hall_1), expose
  `random.calls` via a Level field read in Game::finished — native 4 draws vs
  interpreter 3; with bat's `self.rng.range(random, 3)` made constant, 3=3.
  R05's description stays un-asserted in the dungeon scripted canaries until
  this lands.
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
- [ ] **Proof engine arcs.** Anchoring for machines WITH bodies, induction
  via recursive contracts + decreases, quantifiers, Bag/Seq lowering,
  growing the Lean ladder past L6.
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
  one remaining interpreter/native divergence (R05 description) is the
  non-guard call-chain RNG-stream issue in the backend-residue list (the
  eager-guard half of it is fixed).
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
Still needed (stage 2b+): the decoder, strings/nested/repeated fields,
wire-schemas-as-program-types, runtime layout of wire values, encoding-family
semantics beyond compact_binary v0, and version negotiation.

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

- [ ] Thread refined subslice diagnostics through operator-contract errors once
  `Slice::from/to/range` contracts drive checking directly. (Bounds obligation
  now sources from the spelled operator's `requires` — extend the diagnostics.)
- [ ] Represent length facts and window-shrinking facts as first-class slice
  proof vocabulary (non-empty already exists).
- [ ] Ensure alias and borrow facts understand subslice overlap conservatively.

### Slice Runtime Descriptor Semantics

- [x] Blank-room rendering RESOLVED (verified 2026-06-11): native dungeon
  room lookup/render now produces labels/descriptions byte-identical to the
  interpreter on the canonical scripted loop (the x18 reserved-register fix
  closed the remaining corruption). The only dungeon divergence left is
  R05's data-driven description, owned by the non-guard executor-of-record
  residue bullet — descriptor initialization itself is fixed.
- [ ] Generalize subslice descriptor pointer offsets beyond fixed-array alias
  copy special cases (the `FatDescriptorAbi::subslice` seam exists; widen its
  callers past literal fixed-array bases). Runtime verification is in place:
  all nine `runtime_subslice_*`/nested-subslice canaries are registered
  runtime tests in the suite and differential oracle (verified 2026-06-11).
- [ ] Generalize start-only/end-only/bounded descriptors beyond literal
  fixed-array-backed views.
- [ ] Add focused pass/fail canaries for each newly supported subslice descriptor
  lowering shape as it becomes real.
- [ ] Keep backend reports explicit about descriptor construction and mutation.

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
- [ ] Replace arithmetic-facing proof UX such as `limit - index` with named
  bounded-distance rankings.
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
