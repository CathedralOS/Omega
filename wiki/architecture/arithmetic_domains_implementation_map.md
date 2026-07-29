# Implementation map: arithmetic domains (frozen decision 17)

> **STATUS 2026-07-10: IMPLEMENTED (and past this plan's horizon).** The map
> below is the original 2026-06-14 turnkey plan, preserved for context. What
> landed (see TASKS.md session records 2026-07-09c through 2026-07-09j):
> - The domain rides `TypeReferenceNode::Constrained` and reaches BOTH
>   `RuntimeValueOperand::Binary` (as `arithmetic_domain` + `operands_signed`
>   + a real `byte_width`, exactly the is_float/byte_width threading pattern
>   this plan predicted) and the binary-write ops.
> - Saturating/Trapping emission exists per width (1/2/4/8) on BOTH ISAs for
>   add/sub/mul, signed div/mod's MIN/-1 corner, in STORE and OPERAND
>   position (guard-fused arithmetic clamps/traps with no landing store);
>   64-bit uses flag/high-half witnesses (ADDS+CSINV / SMULH-UMULH on
>   aarch64; carry-overflow cmov / 128-bit one-operand imul-mul + RDX
>   witness on x86_64). Wrapping div/mod's MIN/-1 gets the x86 #DE guard.
> - S2 domain casts (`x as u8 in Saturating`) are RETAGS (the conversion
>   truncates; the tag governs the arithmetic the value joins) and reach the
>   fused-operand witness on both legs.
> - The INTERPRETER mirrors all of it at the operation node (its
>   wide-compute + landing-seam model cannot represent guard-fused domain
>   arithmetic): scalar_locals typed-name map, expression_scalar_type
>   witness (width-promotion-aware, non-Exact tie-break, cast retags),
>   i128/u128 wide compute for the 64-bit widths.
> - Every behavior is pinned by differential RUN canaries
>   (runtime_saturating_*, runtime_wrapping_expression_guard,
>   runtime_divide_min_edge_guard, arithmetic_domain_cast_exit, ...).
>
> OPEN EDGES (see TASKS.md): saturating/trapping SHIFT-LEFT
> (nobody implemented it deliberately; native wraps, interp seam clamps
> stores -- pending/arithmetic/shl_saturating_domain_divergence), shift
> counts at/above width (cross-arch native divergence), range constraints
> under non-Exact domains, S3 Exact proof obligations vs const-fold's
> type-carrying-constants hole, and implementation of the settled float-domain
> model in `float_semantics.md` (`Saturating` clamps finite overflow,
> `Trapping` rejects non-finite results, and `Wrapping` is invalid for floats).

> **Qualification follow-up (2026-07-25).** The language model separates
> unchanged-value domain qualification from numeric conversion. The combined
> `x as Wider in Policy` implementation remains compatibility syntax while
> named width/float conversion operations are introduced. Arithmetic policies
> remain a closed semantic role: attaching one is erased, while conversion and
> later arithmetic carry runtime work.
>
> **Integer-conversion checkpoint (2026-07-28).** Every fixed-width integer pair
> is live in `core::numeric_conversion`: widening only for complete range
> containment, and Exact/Wrapping/Saturating/Trapping narrowing otherwise.
> Cross-signed naming follows representability rather than bit width, so a
> signed-to-wider-unsigned conversion is still narrowing because it excludes
> negatives. Exact is contract-gated; saturation clamps the conversion itself;
> trapping is a runtime event; and every result carries ordinary Exact
> arithmetic. Checked-result narrowing remains design-open; float/integer
> named operations and retirement of compatibility numeric `as` remain. The
> first migration cohort covers indexed operands, guard subjects, comparisons,
> bitwise operands, entry results, 16-bit conversions, and signed/unsigned
> extension. A second cohort covers decimal/binary formatting, decimal parsing,
> FNV hashing, CRC-32, and direct indexed byte writes, with trapping versus
> wrapping narrowing selected by the algorithm. A value-machine call directly
> beneath a value cast or
> qualification is now normalized through a synthetic local, so an indexed
> conversion may be requalified inline inside arithmetic without consuming the
> call scratch slot instead of the delivered result. The user-facing
> `width_mixer`, `array_sum`, `format_number`, `print_number`,
> `multiplication_table`, `prime_sieve`, and `maze_flood` samples now use the
> named surface. A follow-on sweep migrated every remaining runtime integer
> width/signedness conversion in `samples/`; residual integer-looking casts
> there are same-carrier policy qualification or a same-type wire-policy
> compatibility spelling. Because conversion operations are ordinary calls,
> the proof and indexed-range paths consult exact R5 write frames:
> pure/disjoint calls preserve unrelated dominating guards, while opaque or
> overlapping frames still invalidate them. The active PRNG canary cohort now
> names wrapping high-word extraction as well. Runtime branch alias resolution
> follows binary/member argument structure and substitutes its bare parameter
> root, preventing a nested conversion in a mutating value machine from reading
> an unused cloned parameter slot; a focused native canary retains that fix.
> The filesystem consumer cohort now names raw-stat `u8` widening across 15
> native macOS canaries, both filesystem-to-time interop legs, and the Windows
> SetFileTime round trip. The cast-field payload regression now uses the same
> named widening for its raw byte-assembly setup while retaining the final
> payload `mode as u32` that it specifically exists to pin. All 19 reach checked
> trees, and the full native filesystem/GUI suite remains green. Guarded
> nonnegative host counts now use named exact narrowing across the portable and
> target-specific filesystem rows; incoming transition arguments rebind the
> nested conversion contract in checked proof. Each converted count is
> materialized under a payload-distinct local name before enum construction;
> the dynamic native canary and all 88 native filesystem/GUI rows retain the
> delivered count. Target timestamp byte encoders, directory-walk host/count
> conversion, Windows attribute decoding, and portable stat projection now use
> named conversion operations. Remaining filesystem `as` spellings are
> same-carrier Wrapping qualification, target-owned boolean-to-foreign-bit
> encoding, or compatibility-specific lowering shapes. The fixture exposed and
> now pins the backend repair: branch-expanded storage reserves every
> assignment-value call ordinal; leaf-only nested call trees are selectable
> root scopes; top-level
> `Machine::entry` call results match their machine name; and only a bare-call
> local initializer is satisfied by a direct result copy. Embedded calls
> materialize their own result slots before the complete enclosing expression
> is lowered. The `std::time` cohort now names every runtime integer
> width/signedness conversion; residual casts there are same-carrier policy
> qualification or forgetting. Exact ranged locals supply call-contract
> interval proofs, while broader declarations do not. Runtime branching
> substitutes compiler-elided local initializers before enclosing parameter
> aliases and follows cast/call structure; scalar classification recognizes
> the `min`/`max` tree produced by `clamp`. Duration constructors and division,
> clock/sleep, cross-target, and filesystem-time canaries retain the result.

Turnkey entry map for building exact-by-default arithmetic + the
Wrapping/Saturating/Trapping primitive domains. Written 2026-06-14 after the
decision was frozen. Decision text + S1-S4 plan: TASKS.md decision 17; semantics:
chapter_5_expressions_evaluation.md.

## Key reframing (makes S1 mostly type-plumbing)

The current default integer codegen ALREADY wraps at the operand width — the
scalar-width fixes (RuntimeValueOperand::Binary.byte_width) made the emitted
add/sub/mul width-correct, and x86 add/sub/mul wrap modulo 2^width. Therefore:

- **`Wrapping` domain == today's codegen.** No new emission; it is the current
  behaviour, just made explicit/legal at the type level.
- **`exact` (default) == today's codegen + a PROOF OBLIGATION (S3).** Until S3
  lands, `exact` runs identically to `Wrapping` (unsound, but matches today, so
  S1/S2 are non-breaking).
- **`Saturating` / `Trapping` == NEW emission** (overflow detection + clamp /
  trap). The only genuinely new codegen in S1.

So S1's bulk is THREADING an arithmetic-domain qualifier through the type
layers; only Saturating/Trapping add backend work.

## Representation (S1)

Add `enum ArithmeticDomain { Exact, Wrapping, Saturating, Trapping }` (Exact =
default). Carry it ALONGSIDE the scalar primitive — NOT as a ch8 predicate
domain (those are membership predicates like `0..100`; arithmetic domains are
behaviour qualifiers). Candidate homes:
- `PrimitiveType` lives in TWO layers: `omega-symbol-resolved-trees/src/types.rs:618`
  and `omega-typed-trees/src/types.rs:505`. Either add an `ArithmeticDomain`
  field next to the primitive in the type reference, or pair it in the value
  type. Thread it the same way `byte_width`/signedness already flow to codegen.
- It must reach `RuntimeValueOperand::Binary` / the storage-binary-write op so
  the ISA can branch on it (mirror how `is_float`/`byte_width` were threaded —
  see wiki/architecture/scalar_width_rederivation_smell.md for that pattern).

## Parse surface (S1) — SPELLING DECIDED 2026-06-14: `u32 in Wrapping`

Maintainer chose `<primitive> in <ArithmeticDomain>` (reuses the existing `in`
domain spelling). e.g. `count: u32 in Wrapping`, `total: i32 in Saturating`;
domain cast `x as u32 in Wrapping`. Parser entry: the named-type path of
`parse_type_reference_handle` (omega-tokens-to-syntax-trees/parser/type_reference.rs)
— attach an optional `in <Ident>` suffix at the SAME point the optional
`[constraints]` suffix is parsed (~line 195), producing a new
`TypeReferenceNode` shape (e.g. `ArithmeticDomained { base, domain }`) that
threads through symbol-resolved → typed → checked → instruction-selection.
(`in` is the membership keyword; in TYPE position no current grammar consumes a
trailing `in`, so this suffix is additive.) Do NOT parse-and-discard — that is
throwaway; represent it so S2's `as` casts and S3's enforcement can read it.

## Backend (S1)

Arithmetic emission entry (x86_64): `append_runtime_binary_operation`
(isa-x86_64/src/lib.rs ~4637) + `runtime_binary_operation_byte_size` (~4623).
- Wrapping/Exact: existing path (width-correct add/sub/mul/idiv).
- Saturating: op then check OF (signed) / CF (unsigned); `cmov` to TYPE_MIN /
  TYPE_MAX on overflow. Per width + signedness.
- Trapping: op then `jo`/`jc` to a trap (reuse the exit/trap host path, or a
  `ud2`/`int3`). Per width + signedness.
- Mirror in aarch64 or emit a clear "not implemented" until a canary needs it.
- A new storage-write op-kind (if added) must go in BOTH emission-planning
  blocker lists (storage_blockers + runtime_text_blockers) — recurring gotcha.

Interpreter: model all three domains (wrap mask / clamp / trap) so the
differential oracle covers them.

## S2: `as` domain casts + mixed-domain rejection — DONE (2026-06-15)

Domains are **OPERAND-driven** (decision 17): the domain lives on each value's
type, NOT the assignment target. A binary op's domain = combine(left, right)
where `Exact` is neutral (a literal/exact value adopts the other operand's
domain) and a non-exact domain wins. Implemented:

- **Re-key (S2a, 8ec5a447)**: all four binary-write selection sites resolve the
  domain from the OPERAND expressions (`ArithmeticDomain::combine`), not the
  target. Canaries use operand-domained fields (`a: u8 in Saturating`).
- **Mixed-domain rejection (S2c, 43c9b66e)**: omega-validation/arithmetic_domains.rs
  `domain_of` walks value expressions (LocalData init, assignment, terminal) and
  rejects a binary whose two operands carry DIFFERENT explicit domains (recursive,
  so nested mixes are caught). Reads the RAW place type via `declared_place_type_raw`
  (the unwrapping variant stripped the Constrained domain). FAIL canary
  expressions/arithmetic_domain_mixed ("mixed arithmetic domains").
- **`as` domain casts (S2b, 8e7b791c)**: `x as u8 in Saturating` re-tags the
  domain (escape hatch). `domain` field threaded through all four Cast nodes +
  conversions; parser parses the `in <Domain>` suffix; backend + validation read
  the cast's domain. RUN canary expressions/arithmetic_domain_cast_exit.

Interpreter remains local-domain-driven (agrees with operand-driven native when
the local's domain matches the operands' — every canary). operand!=local in the
interpreter is still a documented gap.

## S3: exact enforcement — DONE (2026-06-15)

An exact (undomained) integer `+`/`-`/`*` that is not provably in range of its
type is a compile error. Implemented in omega-validation/arithmetic_domains.rs
(NOT contract_entailment.rs — a self-contained `Interval` engine: operand ranges
from declared type bounds + literal exact values; the result interval is checked
for containment in the result type's range). Wrapping/Saturating/Trapping ops are
exempt (defined overflow); atomic integer types (AtomicU32, ...) resolve as
Wrapping (hardware wrap). `lower_typed_trees` runs `validate_program`, so this
fires for any compiled program including internal test fixtures.

Acceptance met: `nested_i32_mul_overflow_divergence` (pending) → FAIL canary
`expressions/nested_i32_mul_overflow`. Corpus migrated to `Wrapping`
(behaviour-identical, ~37 canaries + 5 differential samples + 1 flow-test fixture
+ 2 missing sample .gitignores). Full workspace green.

## S4: range-inference ergonomics — PARTIAL (2026-06-15)

DONE:
- **Flow-sensitive value tracking** — a per-state-body `ValueEnv` (place ->
  proven interval along the straight-line prefix; updated on LocalData/Assignment,
  dropped on a call) discharges const-init + read-modify-write arithmetic
  (`self.v = 10; self.v += 5`). Cut the S3 blast radius 44 -> 30 pass canaries.
- **Range-constraint narrowing** — `range_constraint_interval` reads a
  `[min..=max]` type constraint (literal bounds) and uses [min, max] as the
  operand interval. So `x: i32 [0..=100]` proves `x + y` in [0, 200] -> exact,
  no domain. Place-arm precedence: flow value > range constraint > type bounds.
  Canary expressions/arithmetic_domain_range_proven_exact_exit.
- **Literal-target folding** — a bounded bare-literal computation is range-checked
  against its destination type (`let c: u8 = 200 + 100` rejected); the target type
  is a fallback primitive used ONLY when the result interval is bounded (so
  unknown operands -- call results -- stay unchecked). Comparison operands carry
  interval [0,1] (not unbounded) so they don't poison enclosing arithmetic. FAIL
  canary arithmetic_domain_literal_target_overflow.
- **Contract `requires` narrowing** — `requires_value_env` reads a machine's
  `requires` comparisons (`amount <= 100`) into the ENTRY state's value env, so a
  bounded param stays exact (`amount + amount` in [0,200]). Canary
  arithmetic_domain_requires_proven_exact_exit.

STILL TODO:
- **Loop / ranking bounds**: a loop counter bounded by `terminates by` could stay
  exact. (The corpus's remaining `Wrapping` operands are mostly call-results /
  cross-state values; those need return-range / inter-state inference, a bigger
  lift -- they read fine as `Wrapping`.)
- **Range-respecting assignment check**: assigning out-of-`[a..=b]` is not yet
  rejected (the narrowed interval trusts the declared range without proving writes
  honour it -- sound as an overflow over-approximation, but a separate obligation).

## PROGRESS

- **S1a DONE** (commit dafcbd8a): the parser accepts `<primitive> in Wrapping`
  (contextual `in` suffix in the named-type path of `parse_type_reference_handle`).
  `Wrapping` is TRANSPARENT (returns the base type) -- correct because the
  integer codegen already wraps at width. `Saturating`/`Trapping` parse but are
  rejected with a clear "not implemented yet" diagnostic; unknown domains error.
  RUN canary `expressions/arithmetic_domain_wrapping_exit` (200+100 u8 -> 44).
  Regression-free, suite 274.

## PROGRESS (S1b)

- **Representation DONE + LIVE** (commits 8fb86b17, e2137582): `ArithmeticDomain`
  in omega-core; `TypeConstraintNode::ArithmeticDomain` threaded through all 3
  type layers + every conversion/consumer; the parser emits `T in Wrapping` as
  `Constrained { base, [ArithmeticDomain(Wrapping)] }`. Validated end-to-end:
  `arithmetic_domain_wrapping_exit` is 70 on interp AND native, so a Constrained
  domain local flows to the backend and resolves to its base primitive.
  (Sat/Trap codegen now LANDED — see "S1b DONE" below.)

## S1b DONE: Saturating/Trapping codegen (2026-06-14)

**Saturating + Trapping are implemented end-to-end on x86_64**, with interpreter
modelling and a differential oracle. Commits 9e583e6f, e6b6fdf9, 87e90e95,
fac0a4ae, e50c3998. Suite 279 green.

How it threads (the byte_width pattern):
- `TypeLayoutDescriptor::Constrained` carries a `domain: ArithmeticDomain`,
  extracted from the type-reference's constraints at the two descriptor-build
  sites (omega-layout/builder.rs, omega-runtime-storage/body.rs).
  `TypeLayoutDescriptor::arithmetic_domain()` reads it (through a leading ref).
- `WriteRuntimeStorageBinary` (abstract + target/assigned op-kind layers) gained
  `domain` + `target_signed`, set at selection from the WRITE TARGET's type
  (`resolve_runtime_storage_arithmetic_domain[_in_table]` + the is_signed
  resolver; the pre-resolved frame-slot path derives from `slot.type_descriptor`).
- x86_64 (`encode_runtime_storage_binary_write`): Exact/Wrapping keep the 64-bit
  op + truncating store; Saturating/Trapping emit a WIDTH-CORRECT add/sub (so
  CF/OF reflect the target width) then a clamp (`append_arithmetic_domain_clamp`:
  unsigned cmovc to UMAX/0; signed mov IMIN + mov IMAX + cmovs + cmovo) or a trap
  (`jno/jnc` over `ud2`). Widths mirrored in `runtime_storage_binary_write_width`
  (+ `arithmetic_domain_clamp_width`, `width_integer_add_sub_width`) for the
  relocation layout; everything rides AFTER the operands so the target reloc
  offset is unchanged.
- Interpreter (`apply_arithmetic_domain` in evaluator.rs): clamp (Saturating) /
  halt (Trapping) at the local write, mirroring native; differential agrees.
- Parser accepts all three domains.

Canaries (canaries/pass/expressions/): `arithmetic_domain_wrapping_exit` (44),
`arithmetic_domain_saturating_exit` (u8 255), `arithmetic_domain_saturating_signed_exit`
(i8 127), `arithmetic_domain_trapping_exit` (in-range 150), and
`arithmetic_domain_trapping_overflow` (native-only abort test
`arithmetic_domain_trapping_overflow_aborts`).

### S1b remaining gaps (deferred, not blocking)
- **aarch64**: Saturating/Trapping emit a clear "not yet implemented" Diagnostic
  (x86_64 only). Wrapping/Exact work on aarch64 (default path).
- **Operators**: only `+`/`-` have Saturating/Trapping codegen; `*`/`/`/etc with a
  non-Exact domain error clearly. Mul overflow detection (CF/OF on imul/mul) and
  div are follow-ups.
- **Field/param targets in the INTERPRETER**: only the LocalData write path applies
  the domain (the canaries use local targets, which the differential covers).
  Native handles any target via the descriptor domain; the interpreter's
  field-write path would need the data member's type-reference domain to match —
  add before differential-testing a field-target saturating program.
- **u64/u64 Saturating/Trapping in the interpreter**: `integer_bounds` returns
  None for those (can't represent u64::MAX in i64), so they fall back to wrap.
  Native is correct (uses CF). Don't differential a u64-domain overflow yet.

## (historical) S1b REMAINING: Saturating/Trapping codegen

Wrapping works (== base codegen). Saturating/Trapping need distinct emission.
OPEN QUESTION to resolve FIRST: is the target's arithmetic domain available at
the binary-WRITE emission site? `byte_width`/`is_float` come from the resolved
primitive descriptor; the domain is an extra CONSTRAINT on the type-reference.
Check whether the type-reference (with constraints) reaches
`resolve_runtime_storage_*` / the WriteRuntimeStorageBinary build, or whether the
domain must be carried onto the frame slot / storage descriptor (like
byte_width). Likely the latter: add an `arithmetic_domain` to the slot/descriptor
when the declared type is `Constrained` with an ArithmeticDomain, then:
1. Thread the domain onto the binary op (RuntimeValueOperand::Binary /
   WriteRuntimeStorageBinary) -- the SAME pattern as the byte_width threading
   (abstract/target/assigned + accessor), set once at build, read by the ISA.
2. x86_64: Saturating = op then check OF (signed) / CF (unsigned) + `cmov` to
   TYPE_MIN/TYPE_MAX; Trapping = op then `jo`/`jc` to a trap. Per width+signedness.
3. Interpreter: clamp / trap to match.
4. Parser: stop rejecting Saturating/Trapping. Canaries: 200+100 in u8 -> 255
   (sat), -> trap (trapping).

`Wrapping` works transparently, but `Saturating`/`Trapping` need the domain
REPRESENTED (not discarded) so the backend can branch. Decided representation:
ride it as a `TypeConstraintNode` variant (e.g. `ArithmeticDomain(kind)`) in the
EXISTING `TypeReferenceNode::Constrained { base_type, constraints }` -- only ~20
TypeConstraintNode match sites vs 59 for a new TypeReferenceNode variant. Steps:
1. Add `enum ArithmeticDomain { Wrapping, Saturating, Trapping }` + a
   `TypeConstraintNode::ArithmeticDomain(ArithmeticDomain)` variant in all THREE
   type layers (syntax / symbol-resolved / typed) + their conversions.
2. Parser: stop rejecting Saturating/Trapping; emit the Constrained type with the
   arithmetic-domain constraint (Wrapping too, so it stops being transparent).
3. Thread the result type's domain to the arithmetic emission (mirror how
   `is_float`/`byte_width` reach `RuntimeValueOperand::Binary` /
   `WriteRuntimeStorageBinary`): the WRITE's target type carries the domain.
4. x86_64 emission: Saturating = op then OF(signed)/CF(unsigned) check + `cmov`
   to TYPE_MIN/TYPE_MAX; Trapping = op then `jo`/`jc` to a trap. Per width +
   signedness. (A new storage-write op-kind must go in BOTH emission-planning
   blocker lists.) Interpreter: model wrap/clamp/trap. Canaries: 200+100 in u8
   -> 255 (sat), -> trap (trapping). This is the multi-crate slice; do it in a
   focused pass and keep each layer green.
