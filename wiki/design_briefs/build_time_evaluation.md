# Design Brief: Build-Time Evaluation — Const Evaluation + Trait Generators

Scouted 2026-06-12 (as "comptime"). Status: DIRECTION SETTLED (2026-07-02, Zach) —
**no keyword, and the term `comptime` is retired as a foreign borrowing** (it is
Zig's name for Zig's staged-metaprogramming mechanism, which this is not).
The concept's name is **build-time evaluation**; staging decisions M1-M6 in
TASKS.md remain the open engineering sequence.

## The settled model (2026-07-02)

- **No marker, ever.** Build-time-evaluability is not a declared property: a
  machine with no declared effects HAS none, so "safe to run during
  compilation" is the absence of effects — already structural, already the
  default. A keyword would restate what the effect system carries.
- **Evaluation time is a fact of the POSITION, not the declaration.** A
  position that needs a value during compilation (an array length, a const
  type argument, a `Layout` policy's `plan()` consumed by the deriver)
  evaluates there; the compiler checks every machine reached is effect-free
  and errors at the use site otherwise, rendering the CHAIN from the position
  to the offending effect (shared diagnostic shape with the abort-authority
  chain).
- **The trait signature is the stability contract.** `Layout::plan` is
  declared effect-free in the trait; conformance already requires signature
  agreement, so an implementation growing an effect breaks at ITS declaration
  — exactly where a keyword would have put the error, with zero new surface.
- **Cross-compilation**: build-time evaluation runs on the host but computes
  TARGET facts — inputs arrive target-resolved (see recommendation 4), and the
  reference interpreter's semantics are target-agnostic.
- The one existing compile-time spelling in the grammar stays: `const N: u64`
  type parameters (a POSITION, on a parameter — never a marker on a machine).

## Current State

- Direction frozen (decision 8 / chapter 14): NO macros, NO #run. Const
  evaluation = effect-free machines in constant positions; trait
  generators = `default machine` bodies using `Self::fields` reflection,
  expanded per conformance, declarer-only, zero effects.
- Landed precedent: Equatable synthesis (equatable.rs +
  structural_equality.rs) — hand-rolled inline expansion at resolved→typed
  lowering. Trait generators would GENERALIZE and eventually replace it.
- The reference interpreter is the candidate engine: effect-free,
  deterministic, differential-oracle-proven; no new evaluation machinery
  needed.
- Const positions today are parse/literal-only — nothing evaluates.
- Decision 12 landed the INFERRED TRANSITIVE EFFECT SURFACE — directly
  reusable as the purity gate.
- First heavyweight client: programmable layouts
  ([`programmable_layouts.md`](programmable_layouts.md)) — the compiler
  invokes `Layout::plan(schema)` at build time and derives codecs,
  projections, and value types from the validated Plan.

## Recommendations

1. **First position: fixed-array lengths** (`[T; N]` where N is an
   effect-free machine call). Lowest coupling, biggest proof leverage
   (lengths drive index facts). Field defaults stage 2; const type params
   stage 3. (The LAYOUTS client needs none of these first: the compiler
   itself invokes `plan()` — a blessed-trait call site, not a general const
   position — so the layouts ladder can start on the interpreter entry point
   alone.)
2. **Purity gate: reuse decision 12** — build-time-evaluable(callee) =
   transitive effects empty AND no &mut/out params. No annotation; the
   position makes it build-time, the effect system makes it legal.
3. **Termination**: NO new rule (maintainer-corrected 2026-06-12; the
   self-recursion framing was Rust-shaped). General recursion does not
   exist in the language — self-calls are tail self-loops and loops carry
   decreases/measures — so build-time-evaluable machines simply inherit the
   existing termination discipline. Fuel at most as a defense-in-depth
   backstop against checker gaps.
4. **Determinism**: emulate TARGET integer widths in the build-time evaluator
   (the interpreter already has signedness/width adjustment — audit and
   reuse). Host-width leakage is a correctness bug; cross-compilation is a
   stage-1 goal.
5. **`Self::fields` exposes names + types only** (stage 1); offsets/case
   reflection deferred. Access spelling: bracket form `self.[field]` —
   syntactically distinguishes the build-time-unrolled access from static
   field access.
6. **equatable.rs is temporary**: stage 2 rewrites `Equatable` as a core
   trait with a generator body and retires the hand-rolled path. One
   mechanism, no special cases.
7. **Failure UX**: compile error at the const site naming the position and
   the failing machine (no silent fallback), with the call chain to the
   offending effect.

## Touches

omega-symbol-resolved-trees (const queries), omega-interpreter (build-time
evaluation entry point + error reporting), resolved→typed lowering
(array-length const pass; generator expansion pass), omega-validation
(witness facts from const results, later).

## Staging

1. Build-time evaluation entry point + effect gate + target-width audit +
   failure diagnostics; the layouts `plan()` call site as the pilot client
   (see programmable_layouts.md) alongside or ahead of array lengths;
   Equatable-via-generator as a hand-wired pilot.
2. Field defaults; general generator expansion framework; Hashable; retire
   equatable.rs.
3. Const type parameters + substitution; decreases-gated recursion;
   const-driven proof witnesses.

## Owner review 2026-07-18: keyword killed, access model confirmed, destructure package

- **The `default` keyword DIES** (owner: "something an LLM dreamed up that I
  was never super sold on"; the house test agrees — the marker earns no
  compiler action). A trait machine either has a body or it doesn't: body
  present = the default, conformance may override. Zero keywords. All
  `default machine` spellings in the record become plain trait-machine
  bodies; ch14's mentions swept with the engineering.
- **The access model is confirmed and named: same visibility, earlier
  clock.** The splice (`self.[field]`) grants nothing the code couldn't
  already touch — the expanded body runs as one of Self's own machines, and
  there is no iterate-all-types anywhere: splice only quantifies over Self,
  inside a trait Self CHOSE to conform to. Conformance is the consent.
  Build-time evaluation runs effect-free machines on VALUES (no type
  tables, no compiler internals, no I/O — decision 12's gate). Contrast
  recorded: proc macros (arbitrary build-time I/O over token streams), Zig
  comptime (type objects), UHT (external parser) are amplified contexts;
  ours is not one. Reflection = iteration + splice, NEVER descriptors —
  a FieldInfo carrying a type would be a type-as-value on the runtime side
  of the universe fence.
- **Equality tiers confirmed**: conformance-generated compare is immune to
  field drift by construction (the splice quantifies over whatever the
  members are); a hand-written equals WINS (landed rule) and is protected
  from drift BY CONVENTION for now (owner: "I'm not going to worry too much
  about forcing exhaustive equality impl yet"). The prover never treats a
  custom equals as substitutable equality — that door is the quotient.
- **Record destructure package (SETTLED, spec direction for the
  data-patterns follow-up):** `let`-position record destructuring is new
  surface, wanted (it proved itself on the canonical-equals sample):
  `let Player { health, gold as g, cached_stats as _ } = self;` —
  bare name binds, `as` renames (colon REJECTED: `field: x` already means
  a type annotation — a pun; `->` REJECTED: saturated, and patterns live
  inside arms), `as _` waives (bind to nothing, visibly ignored).
  **Exhaustive by law**: every field bound or waived — the record twin of
  the landed no-silent-fall-through rule on sums; adding a field breaks
  every pattern that must now decide. Arm-position record binding shares
  the grammar. v1 binding semantics may restrict to [copy]-eligible
  fields (exactly Equatable's existing prerequisite list). Canonical
  hand-written equals opens with the exhaustive destructure — chapter
  convention, NOT enforced.
- **Parked**: a trait-level shape gate ("conformances must open with an
  exhaustive destructure") — held until real drift proves convention
  insufficient. The exhaustive<T>-as-requirable-fact idea was examined
  and rejected on the stratum boundary: our facts are about VALUES;
  "this body read every member" is a fact about CODE, and requiring it
  would need per-field read-effect tracking — the same boundary that
  killed rewrite-registration-as-magic and macros.
- **Binding semantics addendum (owner-reviewed, same arc):** a destructure
  is pure SUGAR for field lets — `let Player { health, gold as g } = self;`
  is exactly `let health = self.health; let g = self.gold;` — and therefore
  inherits the landed let semantics wholesale: **snapshot** (the
  let-capture fix exists because folding across an intervening write was a
  miscompile; copy-vs-view is OBSERVABLE under mutation, not an
  optimization question). For scalars the "copy" is the load the body was
  about to do anyway; the backend folds it when nothing intervenes and
  materializes it exactly when the snapshot is load-bearing. Consequences:
  NO reference-patterns or binding modes, ever (Rust's match-ergonomics
  axis — their multi-year confusion generator — never opens; also
  `&self as Player` cannot mean pattern matching, `as` in expression
  position is the recast). Big fields cannot be silently copied: v1 binds
  [copy]-eligible fields only (exactly Equatable's prerequisite list);
  anything larger is waived (`as _`) or borrowed explicitly by hand
  (`let items: &[Color; 3] = &self.items;` — landed spelling).
- **Still open (the one undecided item):** the unroll spelling for
  generator bodies — the combiner form (`all(self.[field] ==
  other.[field])`) is recommended, unruled.
