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

## Parse surface (S1) — DECIDE THE SPELLING WITH MAINTAINER

Not yet specified in docs. Options: `count: u32 in Wrapping` (reuses `in`),
`count: Wrapping<u32>` (container-style), or `count: u32 wrapping`. The decision
doc says "the value/type lives in a domain" — lean `in Wrapping`. Parser entry:
the type-reference parser (where `: u32` is parsed). Confirm spelling before
building — this is the one open sub-decision.

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

## Suggested first commit (S1a)

Introduce `ArithmeticDomain` + thread it to the binary value operand with
`Exact` everywhere (no behaviour change, full suite stays green) — the plumbing
skeleton. THEN add the parse spelling + Saturating/Trapping emission + canaries
(200+100 in u8 -> 44 wrap / 255 sat / trap), each its own green commit.
