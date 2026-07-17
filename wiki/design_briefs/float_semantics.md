# Float Semantics — the design record (settled 2026-07-18)

This is the settled float model. Chapter 5's Float Facts section is the
user-facing surface; this file records the rationale and implementation laws.

## 1. The architecture: rounded-Rat over target-bound format records

Every serious verification system — Flocq (CompCert's float semantics),
Gappa, SMT-LIB's FP theory, SPARK, LLVM's APFloat — defines a float
operation the same way: `round(format, exact_rational_result)`. Exact
arithmetic composed with one lossy, well-behaved rounding operator,
parameterized by a format record {radix, precision, emin, emax,
special-values policy, rounding rule}. Omega adopts that shape outright,
because the pieces already exist: exact arithmetic is the N2 bignum
engine, finite-floats-embed-exactly-in-Rat is the settled roster fact, and
per-target instruction realizations belong in admitted provider plans (the
landed `provides` rows are the compatibility surface to retire).

The model has three layers:

- **Language** (representation-free): floats are format-parameterized
  rounded-Rat carriers; finite values embed exactly in Rat; comparisons
  may be partial. No NaN, no infinity, no IEEE anywhere in the grammar.
- **Core**: format RECORDS (what Binary32 IS — radix 2, p=24, emin/emax,
  IEEE specials, round-to-nearest-even) are target-independent semantic
  data living in omega::core beside Nat/Rat; the engine's round(format)
  and the proof stratum's decode->Rat consume them.
- **Targets**: provider-plan entries (f32.add -> the FPU instruction) live in
  platform packages selected by the build profile. Hardware entries are
  ACCEPTED-tier (the settled "FPU rounds correctly" permanent boundary); software
  implementations are PROVEN-tier; the trust report shows which.

Rows SELECT among compiler-known lowerings and declare contracts + trust;
they never TEACH the backend encodings (an .omg that emits arbitrary bytes
is an assembler in a costume — the parked inline-asm arc is the honest
surface for that). Today's hardcoded IEEE instruction selection IS the
built-in binding; formalizing it as rows (and the new `Instruction` arm of
the Binding sum) is F7, non-blocking.

**Names mean formats, permanently.** `f32` = IEEE binary32 on every target
that provides it, forever; `p32` = posit32 if it ever ships. A
posit-native target provides p32 in hardware (accepted) and f32 in
software (proven) — the trust polarity flips and the report shows it.
Rebinding `f32` to posits would silently invalidate every proof written
against IEEE contracts: the bleed the design exists to prevent, reversed.

## 2. Domains: the value/policy split

- **Value domains** are unary wellness facts, conjoinable with the landed
  `&`, enforced by invariant windows: `Finite` (not NaN/±inf — ch5's
  `finite` constraint promoted to a core domain), float ranges
  (window-checked facts; every range implies Finite since NaN fails all
  comparisons — one theorem, no ceremony), `Normal`/signed-zero facts
  deferred until a customer.
- **Policy domains** are operation behavior, operand-driven, exclusive per
  op (decision-17 verbatim): quiet default / `Trapping` (trap on
  producing non-finite) / `Saturating` (overflow clamps to ±MAX_FINITE) /
  `Wrapping` = COMPILE ERROR (no modular reading of a float — the Q10
  cast ruling generalized; the Q9 lying-declaration precedent).
- The two axes compose freely (`f32 [0.0..=1.0] in Trapping`): ranges are
  window facts and windows are policy-independent, so no Q9-style lie
  arises (unlike ints, where the range's enforcement mechanism WAS the
  Exact machinery).

## 3. Literals and const-eval: exact Rat, round once

A decimal literal is a rational, exactly. Pipeline: parse -> exact Rat ->
compile-time arithmetic in Rat -> round ONCE at the landing site to the
landing type's format; where the exact op is undefined/overflowing, apply
the format's specials — compile-time equals runtime bit-for-bit by
construction. Constants are unitless until a site requests a type: deferred
typing resolves once at the requesting site, and arithmetic on the anonymous
value is exact.
Conversion vs reinterpretation stays two mechanisms: the value-invariant
mint (`1/3` renders differently per format) is this pipeline; the
bits-invariant read is the recast (`&self.bits as &f32`), footprint-checked,
separate.

One rule retires three pinned residues: FloatLiteral-stored-as-f64-bits
(f32 literal double-rounding hazard), the guard folder computing f32
constants in f64, and the interpreter's missing per-op f32 rounding
metadata. Zig's comptime_float (f128-backed, diverging from runtime modes)
is the cautionary precedent; exact-Rat-then-round is the correct version.

## 4. No ambient relaxation, ever

The graveyard is unanimous. Ambient/implicit looseness: Java's
default-relaxed x87 mode took 23 years to retract (strictfp -> JEP 306);
C's FLT_EVAL_METHOD/x87 excess precision = GCC bug 323, a decade of
double-rounding; FP_CONTRACT-by-default = silent fma, cross-target bit
divergence; -ffast-math deletes isnan checks, breaks Kahan summation, and
(crtfastmath.o) flips FTZ process-globally when a shared library LOADS.
What worked is all spelled at the op or type: Rust mul_add/total_cmp,
Julia muladd vs fma, C23 _Float32, HLSL float16_t. Rules:

- The compiler never contracts `a*b + c`; `fma(a,b,c)` is the
  single-rounding spelling (already in the host Math surface).
- A `muladd`-style "either rounding" op (Julia's middle contract — a
  one-ulp disjunction a prover can discharge) is PARKED until a customer.
- Fast-math-the-flag is not a thing Omega will ever have. Optimization
  permissions, where they ever exist, are per-operation spellings. This
  settles ch5's old "two layers" open note.

## 5. Orders and NaN

- Arithmetic comparison = the format's partial order (landed IEEE
  behavior; false-on-NaN; the folder's type-gated refusal to fold float
  self-compares stays).
- Total order = a NAMED SATISFIER (ch14 machinery, designed for exactly
  this): `sort_by<F64::TotalOrder>` — IEEE totalOrder via sign-magnitude
  integer compare. Rust needed a bolted-on method (total_cmp) and a
  no-Ord-for-floats scar; the satisfier is the honest encoding. Posits
  total-order natively, so their satisfier is a plain integer compare.
- NaN payloads: unspecified after every op, never proof-observable
  (Rust RFC 3514's enumerated-nondeterminism, taken at the contract
  level); recast reads honest bits. `f != f` is demoted to IEEE-binding
  detail; `is_finite` is the portable spelling (posits have NaR and a
  native total order — the idiom never fires there).

## 6. Extensibility ladder (user float types) + the posit future

- Rung 1 (available under today's design): a float type as a LIBRARY —
  encoding-domain carrier + proven software ops with decode->Rat
  contracts. Zero compiler cooperation.
- Rung 2: first-class literals/const-eval by supplying a FORMAT RECORD;
  the engine's generic round(format) covers any fixed-precision radix-2
  format free (bf16, both FP8s); tapered precision (posits) needs the
  record vocabulary to grow one notch, once.
- Rung 3: hardware — provider-plan entries selecting compiler-known lowerings
  (accepted-tier trust, the owner's grant, the report); genuinely new
  instructions wait on the inline-assembly arc.

MLIR's nominal-type-per-format explosion is the warning; APFloat's
semantics-record is the pattern. The quire (posit exact accumulator) is
just data (`[u64; 8]`) when wanted.

## 7. Prior-art constraints

Ada (digits N + model numbers) and Fortran (KIND + inquiry + IEEE as an
opt-in queryable capability module) are the proven
representation-independent architectures — Fortran survived IBM hex, Cray,
and VAX formats. SPARK is the hard datum for proofs: even Ada's
representation-independent design verifies against exact per-target IEEE
semantics, because provers consume exact operation theories — hence:
language parametric, each target binding exact and named. Rust RFC 3514:
pin value semantics exactly, enumerate the NaN-bit nondeterminism, no
ambient fast-math. GLSL precision qualifiers: floor-contracts work but
over-delivering hardware masks under-delivering (test at the floor).
Correctly-rounded transcendentals are production now (CORE-MATH in glibc
2.42+, RLIBM polynomials in LLVM libc, binary32 exhaustively verified) —
"bit-identical math on every target, interpreter as spec oracle" is a
reachable std contract, and our sin/cos (identical op sequence, bit-equal
both engines) is the junior version already. Accelerator reality (bf16,
tf32, two incompatible FP8s, MX block formats, ~a new format yearly)
makes format-as-data descriptive, not speculative.

## 8. Operational edge laws

1. **Min/max NaN contract.** Return the second operand on unordered/equal
   (`a < b ? a : b`,
   matches minsd/FCSEL); order-dependent under NaN, knowingly differing
   from Rust (whose non-NaN-wins LAUNDERS a poisoned value) and IEEE-2019
   minimum (which costs a blend). Under Finite operands all contracts
   agree — proven code cannot observe the choice. Recorded in ch5.
2. **Saturating is overflow-only.** Saturating
   clamps magnitude overflow to ±MAX_FINITE and nothing else; division by
   zero and invalid ops still produce non-finites, which remain Finite
   obligations (cheap, discrete). Division by zero does NOT clamp (0/0
   has no defensible clamp; half-measures rejected). `Finite &
   Saturating` composes (value + policy = different axes) and is the
   ergonomic pairing: magnitude proofs vanish, wellness stays proven.
   Chain rule recorded: any number of value domains, at most one policy
   per `&` chain. The cast's NaN->0 stays cast-specific.
3. **Shift counts use proof-or-policy** (recorded in chapter 5's integer
   section): Exact = count
   provably < width (literal OOR = compile error); Wrapping = masked
   count (the modular reading, hardware-free); Trapping = trap;
   Saturating adds no count meaning (it governs value overflow, not
   operand validity). The ISA's silent masking under Exact is an invented
   number and never adopted. Retires the shift entry of the
   underspecified-numerics family; native==interp by definition.
4. Engineering: the stale bounded_float canary family (the `0.0f` suffix
   no longer lexes) — cleanup rung; the `Instruction` Binding arm (F7);
   format-record vocabulary v1 = fixed-precision radix-2.
