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

- [ ] **Equality/membership split (decision 11).** `x == Command::Move` (bare
  payload-bearing case name) becomes an error suggesting `in`; `x in
  Type::Case` works in value position (`let b: bool = ...`); the guard
  tag-clamp becomes the internal lowering of `in` only; place==place on a
  payload-bearing sum should error until Equatable synthesis (today it slips
  through as a tag/width compare — ACCEPTED known hole, validation cannot
  type places yet); payload-less sums keep implicit `==`.
- [ ] **Pure-discard error (decision 12).** `_ = call();` where the resolved
  callee has an empty effect set AND no `&mut`/out parameters is dead code —
  hard error. Both facts are on the already-resolved signature; cheap.
- [ ] **Property bounds on type parameters (decision 13).**
  `data Box<T [copy]> [copy]` — parse bracket facts on type parameters,
  check them at instantiation, and let the structural copy/send verifier
  accept bounded parameters (it conservatively rejects them today).

- [ ] **Wire stage 2: encoders.** Era-discriminator varint emission (one per
  top-level message, era 0 = pre-versioning body), encoder/decoder
  generation, wire-schemas-as-program-types, runtime layout of wire values,
  encoding families, version negotiation. Differential-oracle-friendly:
  byte-exact expected outputs. (Decision 10 chain checks + migration
  verdicts already landed.)
- [ ] **Versioned data stage 2.** Construction of historical-shape VALUES
  (struct literals currently parse single-identifier type names only — no
  runtime migration canary exists because a `Counter::v1` value cannot be
  built); version MATCH arms (`Counter::v1(old) ->` still rejects);
  migration chains, `replaces`, quiescence obligations; era tag + the wire
  integration decision 10 assumes.
- [ ] **Equatable synthesis / conformance defaults.** Conformance items
  validate written members only; trait `default machine` instantiation and
  the synthesized core derivable set (structural `equals`) are unimplemented
  — this is the compile-time-execution direction (ch13 sketch). Unblocks
  retiring the interim `==` error. Per decision 11: implicit for primitives
  + payload-less sums, `Type satisfies Equatable;` required for structural
  types.
- [ ] **Case members: remaining halves.** Implicit case-domains
  (`self in Type::Case`, unions, exhaustiveness counting), case-subset
  domains, MIXED shapes (common fields + case part). Payload sums are done.

**Backend residue (small, known):**

- [ ] Distinct effectful arm guards: native eager evaluation diverges from the
  interpreter's lazy order (open note in the eager-guard divergence).
- [ ] 3 pre-existing `_compile` canaries hang at runtime (slice-subslice /
  mutable-local family); suite never runs them.
- [ ] aarch64 runtime convergence (dungeon hot-potato).
- [ ] Borrow layer records free-machine value-call targets as `invalid` in
  checked trees (cosmetic today).
- [ ] Stale test fixtures: lib tests of omega-graph/types/names/proof/
  syntax-trees/abstract-operations/target-operations don't compile (missing
  `abi`/`type_parameters`/`kind`/`is_float` fields); omega-machine-emission +
  omega-state-calls each have failing unit tests; architecture_boundaries
  3/6 fail. All pre-date the wave.

**Bigger arcs (Cathedral tier 1, untouched):** concurrency/atomics decisions,
freestanding target + volatile/MMIO, separate-compilation awareness; proof
engine next rungs (anchoring for machines WITH bodies, induction via
recursive contracts, quantifiers).

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
   must_use marker. (Implementation pending: discard-statement validation +
   `_ =` parsing.)
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
    pure non-call expression stays a parse error.
13. **Property bounds: brackets attach to what they follow, everywhere.**
    Type parameters take bracket facts inline: `data Box<T [copy]> [copy]`.
    The Rust-style colon bound (`<T: copy>`) and the attribute-prefix form
    (`[copy]` on its own line) are rejected — colon would split the
    spelling system, and a floating prefix line is positional metadata (the
    attribute magic properties deliberately avoid). Leaves
    `T [copy] satisfies Equatable` room for trait bounds without
    collision.

## Next Up (highest leverage)

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
tag-only case equality in guards; `_ =` accepts only calls.

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
errors. STAGE 2 frontier: no construction of historical-shape VALUES (struct
literals only parse single-identifier type names, so no runtime migration
canary yet), version MATCH arms (`Counter::v1(old) ->`) still reject,
migration chains / `replaces` / quiescence obligations not started, and no
era tag or wire-era integration (decision 10's migration ride).

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
recycling + type-change migration verdicts. Still needed (stage 2):
era-discriminator varint emission, wire-schemas-as-program-types,
encoder/decoder generation, runtime layout of wire values, encoding-family
semantics, and version negotiation.

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
