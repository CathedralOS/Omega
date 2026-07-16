# Design Brief: Build-Time Evaluation — Const Evaluation + Trait Generators

Current design as of 2026-07-18. Omega uses **build-time evaluation**, with no
`comptime`, macro, or `#run` keyword. Staging items M1-M6 in `TASKS.md` are the
remaining engineering sequence.

## The settled model (2026-07-02)

- **No marker, ever.** Build-time-evaluability is derived from the complete
  normalized machine contract. An empty service/operational row is necessary
  but not by itself sufficient: trust reach, authority inputs, resources,
  failure/control outcomes, and termination must also fit the build-time
  context. A keyword would restate a checked contract judgment.
- **Evaluation time is a fact of the POSITION, not the declaration.** A
  position that needs a value during compilation (an array length, a const
  type argument, a `Layout` policy's `plan()` consumed by the deriver)
  evaluates there; the compiler checks every machine reached against the
  build-time contract floor and errors at the use site otherwise, rendering
  the CHAIN from the position to the offending contract axis.
- **The trait signature is the stability contract.** `Layout::plan` has an
  empty published effect row and build-time-compatible contract; conformance
  requires signature agreement, so an implementation growing an effect breaks at ITS declaration
  — exactly where a keyword would have put the error, with zero new surface.
- **Cross-compilation**: build-time evaluation runs on the host but computes
  TARGET facts — inputs arrive target-resolved (see recommendation 4), and the
  reference interpreter's semantics are target-agnostic.
- The one existing compile-time spelling in the grammar stays: `const N: u64`
  type parameters (a POSITION, on a parameter — never a marker on a machine).

## Current State

- Direction frozen (decision 8 / chapter 14): NO macros, NO #run. Const
  evaluation = build-time-admissible machines in constant positions; trait
  generators = trait-machine bodies using `Self::fields` reflection,
  expanded per conformance, declarer-only, and build-time-admissible.
- Landed precedent: Equatable synthesis (equatable.rs +
  structural_equality.rs) — hand-rolled inline expansion at resolved→typed
  lowering. Trait generators would GENERALIZE and eventually replace it.
- The reference interpreter is the candidate engine: contract-gated,
  deterministic, differential-oracle-proven; no new evaluation machinery
  needed.
- Const positions today are parse/literal-only — nothing evaluates.
- Decision 22's normalized row plus the other machine-contract axes supply the
  admission check; empty reach alone is not the whole gate.
- First heavyweight client: programmable layouts
  ([`programmable_layouts.md`](programmable_layouts.md)) — the compiler
  invokes `Layout::plan(schema)` at build time and derives codecs,
  projections, and value types from the validated Plan.

## Recommendations

1. **First position: fixed-array lengths** (`[T; N]` where N is an
   build-time-admissible machine call). Lowest coupling, biggest proof leverage
   (lengths drive index facts). Field defaults stage 2; const type params
   stage 3. (The LAYOUTS client needs none of these first: the compiler
   itself invokes `plan()` — a blessed-trait call site, not a general const
   position — so the layouts ladder can start on the interpreter entry point
   alone.)
2. **Admission gate: reuse the complete normalized contract** —
   build-time-evaluable(callee) requires an empty service/operational row,
   build-time-valid authority/trust/resource/failure behavior, termination, and
   no escaping runtime mutation. No annotation; the
   position makes it build-time, the effect system makes it legal.
3. **Termination**: no separate build-time rule. Recursive calls carry checked
   decreasing measures; runtime lowering additionally requires tail position,
   while proof/build-time evaluation may use measured non-tail recursion.
   Evaluator fuel is only a defense-in-depth backstop against checker gaps.
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
3. Const type parameters + substitution; `terminates by`-gated recursion;
   const-driven proof witnesses.

## Trait-body and reflection rules

- **No `default` keyword.** A trait machine either has a body or it does not: body
  present = the default, conformance may override. Zero keywords. Trait-machine
  bodies use the ordinary `machine` spelling.
- **Same visibility, earlier clock.** The splice (`self.[field]`) grants nothing the code couldn't
  already touch — the expanded body runs as one of Self's own machines, and
  there is no iterate-all-types anywhere: splice only quantifies over Self,
  inside a trait Self CHOSE to conform to. Conformance is the consent.
  Build-time evaluation runs admitted machines on VALUES (no type tables, no
  compiler internals, no boundary I/O). Contrast
  recorded: proc macros (arbitrary build-time I/O over token streams), Zig
  comptime (type objects), UHT (external parser) are amplified contexts;
  ours is not one. Reflection = iteration + splice, NEVER descriptors —
  a FieldInfo carrying a type would be a type-as-value on the runtime side
  of the universe fence.
- **Equality tiers:** conformance-generated comparison follows field changes
  by construction because the splice quantifies over the current members. A
  hand-written `equals` wins and remains responsible for its own field
  coverage. The prover never treats custom equality as substitutable equality;
  quotient laws are the separate mechanism for that claim.
- **Record destructuring direction:** `let`-position record destructuring is
  new
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
- **Binding semantics:** a destructure
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
