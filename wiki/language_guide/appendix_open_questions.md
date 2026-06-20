# Appendix: Open Questions

This page tracks design pressure that is not fully nailed down yet.

## Current Answers

- `const` parameters are compile-time values, proof constants, or both. Omega should use every fact it can soundly know.
- `&mut self` is the working spelling for attached machines and states that need machine state. The goal is not to be different for the sake of being different.
- `machine` is the callable boundary. Calling a machine creates the callable activation; transitioning to an internal `state` does not.
- States are graph labels inside the current machine. They may take arguments and participate in return-value compatibility, but they are reached by `transition`, not by normal call syntax.
- Terminal value completion is useful: a final value completes the active machine with a value, while `transition { _ -> state_name(args) }` jumps to an internal state.
- Relax obligations are compile-time proof obligations. The runtime should not carry hidden invariant state unless a debug/proof artifact explicitly asks for it.
- Target signatures define the invariants they accept. Either the caller can prove the handoff satisfies the signature, or the transition is illegal.
- `domain` names a type-scoped proof predicate over an existing value. Domains
  may have explicit `when` classifiers for cheap domain-pattern matching, but
  Omega should not inject hidden runtime tags to make ambiguous domains
  distinguishable. Matching a data value with `Type::Domain` checks that
  domain and adds its facts to the selected arm.
- `match` is for value selection. `transition` is for control movement. Conditional transitions name a scrutinee; anonymous `transition { _ -> target }` is only for unconditional jumps.
- Borrowed slices should use Rust-like `&[T]` / `&mut [T]` surface syntax.
  They are built-in borrowed views with proof-visible facts such as `len` and
  type-scoped invariant parameters such as `&[T, [non_empty]]`; indexing is
  valid when the current facts prove the index is inside the slice bounds.
- Old bracketed refinement syntax is dead. Range-heavy proof vocabulary lives directly in contracts, using Rust-style ranges like `1..=100` and `min..=max`. The inclusive/exclusive forms are now resolved: `a..b` is exclusive and `a..=b` is inclusive, with `a..=b` normalizing to `a..(b+1)`. Against a length `len`, an exclusive end requires `b <= len` and an inclusive end requires `b < len` (so inclusive-end validity equals index validity); a non-empty inclusive range establishes a `non_empty` fact. These are the same `..` / `..=` forms used for subslicing.
- Text is not a type (decided; see ch8 "Domains On Strings And Encodings").
  "A string" is `[u8] in Utf8`: a byte container (`&[u8]` view, `Vec<u8>` owned,
  or `[u8; N]` fixed) carrying an encoding *domain* (`Slice<u8>::Utf8`,
  `::NoNul`, ...), with the codec as domain-sensitive operators (`+` = concat,
  `[..]` = boundary-checked slice). `String`, `Bytes`, and the earlier
  `Str`/`StrView` are all retired: each named either the byte container
  (redundant with `[u8]`) or the abstract codepoint text -- a quotient of
  bytes-modulo-encoding that has no canonical layout (decoded form is `[u32]` of
  scalar values). The builtin `string` and `String` (`PrimitiveType::String`)
  still exist in the compiler today. A wire `&string` field was prototyped and
  then removed as a vestige: the honest borrowed bytes/text wire field is `&[u8]`
  (already a fat slice natively), riding a raw-byte encoding (length + raw bytes,
  like protobuf `bytes`) distinct from a `[u8; N]` repeated field (packed
  per-element varints). Replacing the builtins with `[u8] in Utf8` wholesale
  waits on domains over `Slice<u8>`, the codec operators, and a corpus migration.
- Surface notation must be honest sugar (decided design law). There are two
  kinds of surface form. TYPOGRAPHY desugars totally and bijectively to the
  nominal core the prover already reasons over, leaving no residue: `[u8]` ->
  `Slice<u8>`, `&T` -> `Ref<T>`, `i32`, `(A, B)`, `1..=10`, `"Hi"`. A LIE
  introduces a type-identity or concept that does not survive desugaring -- a
  "but what is this *really*?" remains: `String`, `Bytes`. Keep typography (it
  is free readability and costs the prover nothing, because it evaporates before
  proof) and reject lies (make the concept justify itself in the open as a
  concept, or cut it). The test for any proposed sugar: desugar it; if nothing
  is left over it is allowed, if a question remains it is a new concept wearing
  sugar's clothing. This is the rule that keeps `[u8]` while cutting `String` --
  and "force everyone to write the canonical `Slice<u8>`" proves too much, since
  run consistently it also deletes `&T`, `i32`, tuples, ranges, and literals,
  leaving the prover's raw terms; the line is "does the notation lie", not "is
  it notation".
- Omega should distinguish proof numbers from machine numbers. `UInt`, `Int`, and `Real` are useful as mathematical/spec types, while `i32`, `u64`, `f32`, and similar types are concrete machine representations with explicit proof obligations.
- Machine integer arithmetic should probably default to exact/proven semantics. Weaker behavior such as `wrapping`, `trap`, `saturating`, or `checked` should be explicit because each mode changes proof obligations and runtime behavior.
- Omega's proof vocabulary should distinguish facts, requirements, guarantees, obligations, invariants, contracts, and boundary. Values carry facts; operations have contracts; contracts create obligations; boundary names the authority for accepting unproved guarantees.
- Termination should be an explicit proof claim such as `terminates`, with
  nested progress clauses such as `decreases value -> OrderOrMeasure`. A
  terminating root like `Main::main` should implicitly require every reachable
  recursive/cyclic path to prove progress through a well-founded ranking view
  such as naturals, slice length, or a named domain/type-specific order.
- Termination syntax sugar and custom ranking views are resolved. The core shape
  stays `terminates { decreases value -> OrderOrMeasure; }`; plain
  `decreases value` uses the default descending-naturals order. Built-in views
  such as `Slice::Length` and descending naturals remain automatic. A custom
  well-founded ordering is declared with a dedicated `measure` keyword as a
  standalone item, not by abusing an `operator` declaration: a `measure` is a
  function from the decreasing value into a well-founded domain such as `usize`,
  e.g. `measure Card::PowerOrder(card: Card) -> usize { card.power }`.
  `lexicographic { a, b, ... }` declares an ordered tuple compared left-to-right
  (e.g. `measure Quest::Difficulty lexicographic { tier, remaining_steps }`), and
  multiple named measures per type are allowed.
- Inline assembly should be parsed as target assembly under Omega's stricter accepted subset rather than bypassing the language. Assembly jumps are only valid if they satisfy Omega's state-transition rules, and assembly memory/register effects must be declared or inferred from known instruction contracts.
- Semantic states remain branch-free. Source-level mid-state transitions may exist for early exits, but the compiler lowers them into generated branch-free sub-states or basic blocks with explicit edges and cleanup.
- A machine enters at the top of its body. Machines may still need target-specific startup rules, but function callability and runtime startup should not be conflated.
- Omega should avoid reserving keywords aggressively. Prefer contextual keywords when grammar position is enough, especially for words like `entry`, `where`, `boundary`, `requires`, and `ensures`. Fully reserved words should be rare and justified by parser clarity, safety, or proof semantics.
- Zero is initialization. The all-zero bit pattern is a valid, memory-safe
  inhabitant of every `data` and `enum` type; invariants and domains describe
  ESTABLISHED values, so a zeroed object carries no facts but is never
  undefined behavior. No niche-style layout optimization may repurpose the
  zero pattern. Enum tag `0` is the first declared variant (declare the
  empty case first). See Memory Layout And ABI.
- There is no separate `enum` type: alternatives are a MEMBER CLASS of `data`
  (`case` members with named payload fields). Member shape determines the kind
  -- fields only is a record, cases only is a sum, both is MIXED (sum-only
  shipped first; mixed is live -- see chapter 1 for the layout,
  zero-unless-named construction, and access rules). Case-bearing data gets the
  full `data` machinery: versions and `wire data` cover the case part, zero
  rules apply uniformly (first case is the zero case). Today's `enum`
  spelling is transitional and retired by this decision.
- Cases ARE domains. A case implicitly declares the same-named domain (tag
  compare as a free classifier); `case` never appears at a use site. Case
  subsets are ordinary domain unions (`when self in Command::Move |
  Command::Say`), replacing shadow enums. Match arms are classifications:
  case arms and domain arms mix in one match with identical `Type::Name`
  spelling, first satisfied arm wins; payload binding is legal only on case
  arms; exhaustiveness counts only decidable arms (cases + pure case-union
  domains), predicate-domain arms require `_`. Subject shape decides
  admissible arms (scalar -> values, record -> domains, case-bearing -> both).
- Cases, domains, and machines share one `Type::member` namespace; any
  collision is a hard compile error -- no shadowing, no resolution priority
  (silently rebinding a match arm's meaning is never acceptable). Domains MAY
  be declared over foreign types (extension-trait analog), import-gated, with
  the same loud-collision rule; upstream additions that collide are loud
  breaking changes caught at whole-program compile (later: package admission).
- Atomics are Rust/C11-like: dedicated core atomic types with explicit
  orderings (`Relaxed`, `Acquire`, `Release`, `AcqRel`, `SeqCst`) on every
  operation; the sanctioned shared-mutation carve-out from exclusive `&mut`.
  Working assumption: compiler intrinsics, not boundary operators. See
  Concurrency.
- Volatile/device-memory access goes through boundary operators with volatile
  contracts (each access exactly once, declared width, program order among
  volatile accesses on a region), not a type qualifier. Hardware ordering is
  the boundary contract's job, not volatility's. See Memory Layout And ABI.
- Freestanding (ring-0) targets have an EMPTY host-provider set; the lowest
  boundary declares facts about hardware (MMU, MSRs, MMIO regions, interrupt
  control), and the asm instruction-contract subset is the implementation
  vehicle for many such providers. See Capabilities, Effects, And Boundaries.
- Type PROPERTIES and traits are distinct classes with distinct spellings:
  properties are lowercase facts in brackets on the data declaration
  (`data Point [copy, zero_init]`, reusing invariant-parameter syntax;
  computed / declared+verified / boundary-asserted, no inference, no negative
  form); traits are behavior, implemented by ordinary machines and claimed by
  a standalone conformance item `Point satisfies Equatable;` (checks written
  members, instantiates trait default-machine bodies, synthesizes core
  derivable traits). Nothing trait-shaped appears on data declarations.
- Zero-is-initialization splits: "zero is a valid value" is the unconditional
  compiler guarantee (no niche layout, tag 0 = first case); "zero is the
  semantically empty value" is the opt-in `zero_init` property whose
  verification owns the zero-case-payload-free rule. Cathedral requires the
  property on its surface types; the language does not impose it.
- Equality is trait-resolved: core `Equatable` with compiler-synthesized
  structural `equals` (field-by-field; tag + matching payload for sums).
  Interim, until that lands: `==` on payload-bearing case values is a compile
  error (never a tag-only comparison); payload-less sums keep `==` as the tag
  compare.
- Synthesis is a closed compiler privilege today (the Slice::index pattern:
  browsable core declaration, compiler-owned implementation). NO macro
  system, ever, and no `#run` directive: compile-time execution is what two
  language surfaces MEAN -- const evaluation (effect-free machines in
  constant positions evaluate at compile time; the position is the trigger)
  and trait generators (member reflection inside trait-declared
  `default machine` bodies, expanded at conformance sites; build-time code
  runs only where the trait declarer wrote it and must be effect-free). The
  reference interpreter is the engine for both. Once generators land, the
  privileged core set dissolves into ordinary core traits.
- Attribute-system stances: no per-item conditional compilation (per-target
  code lives in target packages); lint policy lives at the package/build
  declaration, never per-item in source; deprecation is versioned-data
  metadata, not a marker; field-level codegen metadata is wire data, not
  attributes; optimization hints deliberately deferred. `[open]`
  (non-exhaustive evolution contracts) is DROPPED until separate compilation
  gives it teeth; an in-language test surface is deferred.
- Wire compatibility rulings: declared `version` blocks are checked schema
  history -- violating them (including retiring a documented field number
  without reserving it) is a compile error; schemas without version blocks
  get only self-contained checks. Compatibility runs along the version chain
  (adjacent eras), not all-versions-vs-current. Wire schemas are ordinary
  data (native in-memory layout, usable in any signature -- the chapter's
  examples already do this) that additionally carry the wire schema;
  encode/decode is generated at boundaries. Version type identity moves from
  display-string comparison to symbol identity when validation does
  generally.
- Era discriminator + recycling: generated wire encodings always carry one
  era varint per top-level message/record (never per struct, never in
  memory); no-version schemas encode era 0, and adding versioning later
  snapshots the old body as that era. Cross-era field-number recycling is
  legal (era tables disambiguate); cross-era type changes report "requires
  migration" instead of erroring. Cross-binary case openness is a wire
  decode policy for unknown case tags (reject / preserve / decode as the
  zero case), never a weakening of in-language exhaustiveness -- the
  `[open]` property is permanently dropped, subsumed by wire.
- Discarding a non-unit return value is a compile error; intentional
  discards are spelled `_ = call();`. No per-type must_use marker exists --
  the default is the strict behavior. Refinement (frozen decision 12): the
  gate is "evaluation has effects", not "is a call" -- effectful boundary
  operators (volatile/MMIO reads) become discardable when they exist, and
  discarding a provably PURE call (empty effect set, no `&mut`/out
  parameters -- both signature-level facts) is a hard error, since it is
  dead code.
- Equality vs membership (frozen decision 11): `==` is value equality
  through core `Equatable`; `in` is domain membership (tag test, legal in
  value position, lowers to the tag compare). A bare payload-bearing case
  name is not a value: `x == Command::Move` errors suggesting `in`; the
  brace form compares structurally. Equatable is implicit for primitives +
  payload-less sums, declared (`Type satisfies Equatable;`) for structural
  types; adding a payload case flips implicit -> declared, re-erroring `==`
  sites deliberately.
- Generic property bounds (frozen decision 13): brackets attach to what
  they follow at every position -- `data Box<T [copy]> [copy]`. Colon
  bounds and attribute-prefix lines are rejected; trait bounds compose as
  `T [copy] satisfies Equatable`.
- Version matching (frozen decision 14): a builtin `Versioned<T>` container
  (`{ era: u32, payload: union-of-eras }`), minted only at boundaries, is
  the ONLY legal subject for version match arms -- plain values never carry
  era tags (the per-struct tag tax stays rejected). `era` is read-only
  queryable; the paren arm form binds the whole historical value;
  migration-chain completeness is a report verdict.
- Lifetimes (frozen decision 15): the Rust model, adopted wholesale --
  tick-spelled lifetime parameters in the ordinary `<>` list, elision for
  the common cases (single ref input; `&self`), borrow-carrying data
  in-model (`data ChatMessage<'buf>`). Rejected spellings: `from`/`borrows`
  clauses (cannot name a struct field's source), keyword region/origin
  parameters (verbose ambiguity), Mojo bracket origins (collide with
  slice/property/invariant brackets). House style: descriptive names
  (`'buf`), never `'a`.
- Suspension (decision 16, AMENDED 2026-06-13 in chapter 17): typed state
  clusters CAN suspend across ticks; no `async` coloring, no `Future`.
  Waiting is an ordinary CALL to a boundary wait primitive (a futex-shaped
  `Scheduler` trait: wait-on-word / wake-N -- the ONLY wait mechanism,
  ever), MARKED with `await` at the call site; the compiler requires
  `await` on any call carrying the `suspend` effect (call-site visibility
  marker, NOT signature coloring). HARD RULE: suspend-in-call is forbidden
  -- a `suspend` machine can be SPAWNED but not CALLED, so suspension never
  nests through a call chain and a parked task's carry-set is single-level
  (M = max over its own await points; N derived from the resource parked
  on, so `M x N` is a model-checked bound). Borrows may not live across an
  `await`; effect ceilings forbid `suspend` in ISR-like contexts; scoped
  spawns borrow with no scope keyword (drop of `Join<T>` joins);
  cancellation is a value at the wait (zero case, no unwinding, rides
  chapter 15's recoverable channel); there is NO select -- producers post
  into one mailbox carrying a case-bearing sum (Erlang's model). The
  earlier no-keyword form is superseded. See chapter 17 +
  wiki/design_briefs/concurrency_atomics.md.
- Ranking views (decided 2026-06-12): the use-site subtraction
  (`decreases limit - index`) is rejected as permanent surface; the
  argumented tuple form `decreases (index, limit) -> Nat::BoundedDistance`
  is the blessed spelling (the arrow's left side is uniformly the ranked
  subjects) and the subtraction form retires once it lands.
- Concurrency scheduler strategy (decided 2026-06-15): Omega owns the
  language MODEL (stackless spawned-machine suspend/resume, the
  `Send`/`Share` data-race discipline, the memory model, atomics, and the
  `Scheduler` INTERFACE), NOT a concrete scheduler. Hosted borrows the OS
  scheduler (1:1 threads); Cathedral provides its own. A scheduler is NOT
  required for any SAFETY promise (data-race / deadlock / protocol freedom —
  provable against every scheduler incl. adversarial); LIVENESS (progress /
  no-starvation) is only ever a theorem CONDITIONAL on fairness (adversarial
  progress is logically impossible), discharged by trusting the OS or, later,
  an owned cooperative scheduler. Decisions: structured concurrency;
  non-multi-copy-atomic targets (POWER) off the list; a restricted static-task
  sub-language for the Cathedral kernel; `Sync` renamed `Share` with a
  gradual-loosen ladder (L0 move-only -> L1 sync-wrapper -> L2 immutable share
  -> L3 proof-checked disjoint MUTABLE share); enforced real-time deferred. See
  wiki/design_briefs/concurrency_atomics.md (2026-06-15 review) for the full
  catalog, corrections, open questions, and build order.
- Concurrency follow-up resolutions (2026-06-15 working session, see
  concurrency_atomics.md "Working-session resolutions"): the share-capability is
  named **`Share`** (symmetric verb with `Send`; `Sync` dropped). **Preemption =
  safe-point** (compiler-inserted yield-checks at back-edges/transitions; keeps a
  suspended task as data-at-a-known-point, preserving bounded/overflow-proof state
  — full async preemption deferred to hard-real-time only). No recursion → WCSU
  bounded stacks → **stack overflow impossible by construction.** Suspension lives
  at explicit **`await` call-sites** (a state suspends by awaiting its mailbox/IO
  machine; the high-level FSM stays coloring-free). **Device memory** is a
  capability-gated `Mmio<T>` whose abuse-prevention is the VM mapping (grant =
  install the PTE), with mapped (trusted, no-trap) vs mediated (sandbox/emulated)
  lowerings and shared-ring/poll to avoid trap storms. **Reclamation** = drop/RAII
  for owned data (GC rejected); lock-free quarantined to one channel primitive +
  an SMR scheme. Pointers need no `unsafe` (borrow obligations already cover the
  sequential case). Oracle = confluence + seeded scheduling. Concurrent-structure
  substrate = **generational arena handles** (`{index, generation}`, no raw
  pointers; deref is bounds+generation checked, fail-safe — kills allocator-UAF
  and ABA by construction; same model as Cathedral's grant arena), **paged** for
  unbounded growth (immovable pages, lock-free CAS-linked).
- Recoverable errors, traps, and failure (frozen decision 18, see chapter 15):
  recoverable failure is an ordinary **sum handled by an exhaustive transition** —
  no dedicated error type, no `?`, no propagation sugar. The must-use /
  forced-handling / no-success-payload-on-failure guarantees are corollaries of
  strict-use (decision 9) + exhaustive transitions + case-payload binding, not
  error-specific machinery. Propagation is an explicit arm targeting a state that
  returns the caller's own failure (verbose by design). **Success cases carry
  proven facts** via the callee's `ensures` (`ensures Found.index in 0..16`),
  inherited by the handling arm — the first instance of the unified fact catalog
  (see below). There is **no trap category for logic**: proven-impossible states,
  failed exact-arith proofs, and provable contract breaches are COMPILE errors;
  prover incompleteness forces an explicit handler or an explicit abort, never a
  hidden runtime trap. The only opt-in in-language runtime trap is `T in Trapping`
  arithmetic (decision 17, done). No `expect`/`unwrap` — prove the failure case
  dead instead. Deliberate termination is the **`abort` effect**: contagious
  (infects `main`, callers, and boundaries; package-policy-visible), nuclear (no
  cleanup, no unwind, lowers to an `exit`/`abort` boundary call). Graceful
  shutdown is NOT abort — it is explicit cleanup states + `exit(code)`. Host
  failure returns a sum (out-parameter result types rejected as surface). Failure
  cleanup is an ordinary per-edge drop set (chapter 16), not special machinery.
- Unified fact catalog (frozen direction, decision 18 / chapter 15 + chapter 7):
  the borrow checker, the arithmetic-domain interval engine (decision 17 S4), and
  sum case-narrowing converge into ONE flow-sensitive fact catalog threaded
  through the CFG, carrier-generic (scalar→interval, sum→which-case,
  slice/`[u8]`→length+encoding, ref→validity). Guards and case partitions narrow
  by intersection (tightest wins). Cross-call propagation is **modular and
  contract-mediated** (prove the callee's `requires`, assume its `ensures`), with
  contracts **inferred** within a compilation unit and **written** only at
  boundaries (exported APIs, separate compilation, recursion) — chosen over
  whole-program context-sensitive propagation so separate compilation survives.
  This answers the open question below ("infer result bounds from match/transition
  partitions"): yes, via the catalog; the v1 useful fact-kinds are intervals
  (done) + which-case + slice-length. The remaining open spelling is how a fact
  attaches to a case payload field (`ensures Found.index in 0..16` is the working
  sketch).

## Still Open

- Concurrency residuals (see concurrency_atomics.md): can the proof system prove
  **L3 disjoint MUTABLE sharing** (region/capability) for lock-free mutable
  aliasing without a runtime SMR scheme — the "safe-surface lock-free" frontier?
  The **`Scheduler` interface** details (`wake_one` vs `wake_all` default; the
  FAIRNESS promise — a borrowed OS futex is not fair, so liveness hypotheses built
  on it may not hold; timed-wait placement). The exact **device-memory source
  surface** (type spelling, ordering args). Only if hard-real-time is ever
  promised: the full-async-preemption register/stack context-switch design
  (otherwise dissolved by safe-point preemption). And **OQ8** — whether Omega can
  ever ADMIT "known-good" raw-pointer lock-free algorithms (true Treiber etc.) it
  currently prunes: NOT a Gödel "true-but-unprovable" (they ARE provable, in
  Iris-class logics) but the soundness⇄completeness tradeoff (Rice) — a sound
  decidable checker MUST reject some safe programs, so the question is only how far
  up the proof-power ladder we climb to shrink that set (never to zero). The
  generational/paged-arena substrate covers the practical need meanwhile.

- ~~Can the compiler infer result bounds from `match` and `transition` partitions without explicit annotations?~~ RESOLVED (decision 18 / unified fact catalog, above): yes — a transition partition narrows facts per arm via the same catalog the arithmetic domains use; cross-call propagation is modular/contract-mediated with intra-unit inference.
- How much domain classifier/checker inference should Omega attempt beyond
  explicit `when` clauses and executable domain bodies?
- What exact source form should core operator declarations use for `[]`,
  subslicing, arithmetic, and string concatenation?
- Which core semantic types should be browsable source declarations, and which
  primitive carriers should remain compiler-managed? Current direction:
  `Array`, `Vec`, and `Slice` (over `u8` and other elements) are the public core
  containers; text is `[u8] in <encoding domain>` rather than a `String`/`&string`
  concept of its own; `Ptr` and descriptor-like carriers sit at the low-level
  boundary.
- How should Omega express and prove sequence-wide domains over runtime text
  (decided direction; see ch8 "Domains On Strings And Encodings")? Encoding
  validity is a domain over the byte container (`Slice<u8>::Utf8`, `::NoNul`,
  ...); the byte-level proof tax is avoided by validating once at the ingest
  boundary (a `when valid_utf8` classifier/checker), carrying `in Utf8` as a
  fact, and discharging a few preservation lemmas as operator contracts (concat
  and boundary-slice preserve UTF-8) so downstream code never re-scans. Richer
  sequence-wide invariants are staged later.
- When domains can participate in operator resolution, what exact ambiguity
  rules should apply, and which concepts should remain ordinary value domains
  versus a separate evaluation-mode/policy system?
- Should relax ever permit graph-edge proof debt, or should it remain strictly
  lexical and non-transitioning?
- How explicit should weakened machine invariants be in target state signatures?
- What syntax should Omega use for float optimization permissions, separate from float invariants?
- Which float properties should be first-class invariants: `finite`, `non_nan`, `normal`, signed-zero policy, or something else?
- What exact spelling should arithmetic modes use: scoped policy, operator variants, or domain-sensitive operator resolution for semantic quantity domains?
- Does `checked` arithmetic return `Option`, `Result`, a language-specific checked value, or require explicit operator forms?
- How should `Real` contracts lower when called with `f32` or `f64`: explicit `approx<Real, eps=...>`, compiler-inferred error bounds, or named approximation policies?
- Should inline assembly allow local labels and internal jumps, or only structured exits that map to Omega transitions?
- What is the minimum contract syntax for assembly clobbers, memory effects, target features, and emitted invariants?
- Which assembly instructions belong in the first accepted subset for each target, and what exact contracts should each instruction emit?
- When should manual assembly contracts be allowed to supplement known instruction contracts, and when should they be rejected as too opaque?
- Which words must be globally reserved, and which should remain contextual keywords only?
- How should imported library/syscall signatures and target bindings describe native operand lowering, so `Stdout.write` and `Process.exit` are not compiler-special string matches?
- Should the language eventually support explicit tail calls into machines, and if so what spelling avoids confusing them with ordinary `-> state()` transitions?
- Case members: payload-binding spelling in `transition` vs `match` arms
  (expected to reuse data-destructure guards), generic payloads, the exact
  tag-prefixed overlay layout, and whether case-union domains are recognized
  for exhaustiveness syntactically or by classifier analysis.
- Foreign-type domains: the import-gate spelling, whether an orphan-strict
  mode (domains only in the owning package) is offered, and how
  foreign-declared domains surface in authority-flow reports.
- Zero-is-initialization follow-ups: the zero-excluding-invariant lint, and
  whether any type may opt into constructed-only semantics.
- Atomics follow-ups: standalone fences, `compare_exchange` failure-ordering
  surface, and how the concurrency proof model treats relaxed visibility.
- Volatile/hardware follow-ups: the operator surface for MMIO regions, `repr`
  spelling for packed/explicit-offset hardware structures, untagged unions.
- Freestanding follow-ups: target-declaration shape for "no host", the entry
  provider contract (firmware handoff state), interrupt-handler entry into a
  machine graph, section/physical-address placement in image emission.
- What is the component artifact + cross-component ABI for separately
  compiled, hot-swappable machines? (Inventory of current whole-program
  assumptions: wiki/architecture/whole_program_assumptions.md.)
- Type-property follow-ups: the bare generic-bound spelling (likely
  `where T is [copy]`), the initial core property set (copy/zero_init/send/
  sized), and whether `[open]` (non-exhaustive evolution contract) and
  `[must_use]` join the bracket surface.
- Conformance-item follow-ups: identifier-led item parsing, the both-foreign
  orphan rule, partially-satisfied diagnostics.
- An in-language test surface (a `test` item word the toolchain discovers) is
  undesigned; canaries are external today.
- Compile-time execution follow-ups: the member-reflection surface
  (`Self::fields`, the field splice, reflection over cases/payloads), which
  positions count as constant for const evaluation, and how the proof system
  sees generator-expanded bodies.
