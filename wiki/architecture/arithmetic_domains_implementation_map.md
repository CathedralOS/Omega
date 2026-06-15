# Implementation map: arithmetic domains (frozen decision 17)

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

## S2: `as` domain casts + mixed-domain rejection

`as` already exists for numeric casts (Cast node + Convert operand). Extend it
to cross arithmetic domains (`x as Wrapping<u32>` / `x as u32` exact). Add a
typecheck rule: a binary whose operands disagree on arithmetic domain is a hard
error (no mixed-domain), with a diagnostic pointing at the `as` fix.

## S3: exact enforcement (the big breaking slice)

The range/entailment prover (omega-validation/contract_entailment.rs — already
does intervals + difference-bound matrices) must discharge "result in range" for
every Exact arithmetic op; unprovable = compile error directing to widen (`as`)
or pick a domain. Migrate samples/canaries as fallout (accepted). Acceptance:
the existing nested_i32_mul_overflow_divergence canary becomes a FAIL canary
(rejected at compile time).

## S4: range-inference ergonomics

Discharge common bounded cases (literal const-fold range checks, field/param
ranges from contracts/domains, loop bounds from `decreases`) so S3 is not
annotation-hell.

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
- **u64/usize Saturating/Trapping in the interpreter**: `integer_bounds` returns
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
