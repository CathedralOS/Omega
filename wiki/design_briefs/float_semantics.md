# Float Semantics — the design record (settled 2026-07-31)

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
engine, finite nonzero floats embed exactly in signed Rat, and
per-target binding tables are the landed bootstrap mechanism being
migrated to explicit conformances from which the toolchain derives typed
`ProviderPlan` artifacts.

The model has three layers:

- **Language** (representation-free): floats are format-parameterized
  approximation carriers; finite nonzero values embed exactly in signed Rat,
  signed zero/infinity/NaN live in the proof-level `FloatMeaning` sum, and
  comparisons may be partial. No target instruction or ambient mode is part of
  the grammar.
- **Core**: format RECORDS (what Binary32 IS — radix 2, p=24, explicit
  normal/subnormal exponent bounds, IEEE specials, round-to-nearest-even)
  are target-independent semantic data living in omega::core beside Nat/Rat;
  `FloatFormat::BINARY32` and `FloatFormat::BINARY64` are now ordinary core
  constants. The engine's `FloatSemantics` functions and the proof stratum's
  bit-pattern-to-meaning decoder consume them.
- **Targets**: checked software machines and checked target instructions
  (`f32.add` -> the FPU operation) live in target packages as explicit
  conformances. Irreducible target operations use parsed, contract-emitting
  `asm { ... }`; the toolchain derives the selected `ProviderPlan`. Hardware
  semantic claims are admitted because the vendor supplies the silicon fact;
  checked software implementations may be derived against the same executable
  contract. The trust report distinguishes them.

Rows SELECT among compiler-known lowerings and declare contracts + trust;
they never TEACH the backend encodings (an .omg that emits arbitrary bytes
is an assembler in a costume — parsed checked assembly is the honest surface).
Today's hardcoded IEEE instruction selection is the bootstrap binding. F7
migrates it to target conformances plus the checked instruction catalog. The
former `Binding::Instruction` bootstrap carrier is already retired.
The x86-64 backend now has exact register-only VEX encoders for scalar
`VFMADD132SS` and `VFMADD132SD`, but their existence is instruction mechanics
only. The adjacent opt-in admission rung requires one exact deployment profile,
the canonical AVX+FMA3 pair, and Binary32/Binary64 raw-bit cancellation receipts
that distinguish fused results from multiply-then-add. Its opaque provider
carrier binds both generic FMA slots to the exact scalar instructions and is
the only route by which the existing source-free custody seam may enter final
image emission. Exact-target `build.omg` activation now selects the closed
`Baseline` or `AvxFma3` deployment value and retains an admitted provider on the
checked compilation, with targetless, non-x86, and cross-profile substitution
failing closed. Linux and Windows x86 target sources now publish the two
nearest-FMA satisfiers, and checked compilation joins each actual source use's
complete selected `ProviderPlan` and exact compiler-intrinsic provenance to the
matching admitted provider. Repeated uses deduplicate, F32/F64 remain separate,
and multiply-then-add or non-x86 plans cannot borrow the admission. This is
checked custody, not native execution evidence, and does not widen the generic
SSE2 x86-64 targets: compiler/native lowering that consumes the association
remains pending.

**Names mean formats, permanently.** `f32` = IEEE binary32 on every target
that provides it, forever; `p32` = posit32 if it ever ships. A
posit-native target provides p32 through an admitted hardware semantic claim
and may provide f32 through a derived checked software realization — the trust
polarity flips and the report shows it.
Rebinding `f32` to posits would silently invalidate every proof written
against IEEE contracts: the bleed the design exists to prevent, reversed.

### Ordinary operator resolution, not float magic

Primitive float spellings use the same operand-driven operator resolution as
integers and authored semantic-facet domains. There is no float-only resolver,
runtime operation tag, or `FloatOperations<F>` bundle.

```text
left + right
    -> resolve the complete static carrier/domain tuple
    -> select the named Float::add requirement
    -> select a target satisfier and normalized ProviderPlan
    -> realize ADDSS, FADD, or a checked software machine
```

The format comes from the permanent carrier identity. The domains qualify the
operation. Selection is static; ordinary arithmetic never forms `dyn` and
carries no runtime table.

Arithmetic, comparison, conversion, classification, multiply-then-add, fused
multiply-add, and directed-rounding operations have separate normalized
requirement identities. In particular:

```text
multiply_then_add = round(round(a * b) + c)
fused_multiply_add = round(a * b + c)
```

The compiler cannot contract the first into the second because they are
different operations, not alternative realizations of one requirement.
Nearest-even is the ordinary arithmetic contract. Toward-zero and the other
directed modes are separately named operations, never ambient control state.

### One executable semantic definition

The public contract is an equality against an executable core function rather
than prose or a four-place relation:

```omega
data FloatMeaning {
    case FiniteNonZero(value: Rat::NonZero);
    case Zero(sign: Sign);
    case Infinity(sign: Sign);
    case NaN;
}

FloatSemantics::add(
    format: FloatFormat,
    left: FloatMeaning,
    right: FloatMeaning,
) -> FloatMeaning;
```

This is ordinary core proof data.
`Rat::NonZero` is the checked `num.pos != num.neg` domain;
`nonzero_rat(negative, positive, denominator)` establishes it from direct Nat
premises, and the finite case accepts only the qualified result. Because the
sum contains proof-only `Rat`, attempting to place `FloatMeaning` in runtime
storage is rejected rather than silently inventing a tagged runtime ABI.

Conceptually, the boundary operator promises:

```omega
ensures meaning(result)
    == FloatSemantics::add(format_of(result), meaning(left), meaning(right));
```

`FiniteNonZero` excludes rational zero so the sum has no overlapping
representations. Subnormals are ordinary nonzero signed rationals. Signed zero
remains separate because operations such as reciprocal observe its sign.
Infinity and NaN have no rational embedding. NaN payloads collapse only in the
proof stratum; the runtime carrier retains its honest bits.

D40 fixes equality as structural equality of this proof-only sum. Payload
erasure has already happened when the runtime carrier projects to the single
payloadless `NaN` case; equality performs no second erasure. Consequently NaN
is reflexive and the two `Zero(sign)` values remain distinct. This is not IEEE
`==`: that separate atomic runtime relation makes NaN non-reflexive and signed
zeros equal.

Each projected term retains a verifier-reconstructible landed float source,
exact format, exact `meaning32`/`meaning64` operation, and exact recognized-core
declaration plus numeric-catalog contract. Equal tuples canonicalize to one
proof value; source occurrences and spans remain separate provenance. Distinct
source terms require an explicit theorem and never coalesce by spelling,
fingerprint, ordinary IEEE comparison, or coincident value. The entire carrier
is PCC metadata and has no runtime representation.

The same semantic function is consumed by proof checking, build-time folding,
the interpreter, checked software realizations, and target-validation tools.
This replaces the current folder path that can evaluate landed `f32` operations
through host `f64`: the semantics becomes the folder, so build-time and runtime
cannot acquire independent definitions.

The current shared engine defines binary32/binary64 decoding, exact rational operations, rounding, classification, square root, conversions, directed rounding, and distinct fused and unfused arithmetic. Folding, interpretation, checked operation evidence, and backend guard folding consume that one definition. Proof projection may erase NaN payloads while runtime carriers preserve exact representation bits. Target realizations remain independently validated; incomplete provider and proof work is tracked in `TASKS.md`.

### Public conversion family (settled 2026-07-31)

Policy-bearing conversion uses destination-owned requirement identities:
`F32::from_f64`, `F32::from_i64`, `I32::from_f64`, and the corresponding
primitive-destination families. A generic consumer names the exact operation it
needs through an ordinary one-off machine bound, so the family needs no
universal `Convert<From, To>` trait.

The default format and integer-to-float operation rounds nearest-even. The
default float-to-integer operation rounds toward zero; fractionality is lossy,
not a failure. Its unqualified result is proof-gated on a finite input whose
truncated result lies in range. `Trapping` and `Saturating` are result-domain
overloads of the same operation identity, while `Wrapping` has no candidate.
The overload result retains the selected policy; callers explicitly erase it
when subsequent arithmetic should return to Exact.

Named-machine result dispatch is a general domain rule rather than float
magic. For one path and parameter signature, the expected result's normalized
dispatch-bearing domain set must equal one declaration's result set. With no
expected result, the empty set selects the unqualified overload. Predicate-only
knowledge does not dispatch and is proved after selection. Semantic-role
contributions, routed provenance, and empty explicit tags do dispatch; mixed
domains still owe their predicates. Duplicate dispatch sets reject at the
declaration, and the normalized set enters requirement and artifact identity.
Fixed operator spellings remain operand-directed.

Directed one-step conversions remain separately named requirements and never
read an ambient/runtime rounding mode. Ordinary library wrappers may compose
rounding and conversion when their contracts prove the same meaning; a
one-step `FloatSemantics` conversion is required where composition would double
round. Bare float-format conversion is total, including infinity; refinements
such as `Finite` are later predicate obligations, not conversion variants.
Same-format policy qualification belongs to `as`, not this family.

A failure-returning conversion changes result shape. Its public name and
carrier stay deferred with checked-result arithmetic; this does not block the
ordinary, trapping, saturating, format, or integer-to-float requirements.

The user-facing CLI sample corpus now consumes this named family for every
integer-to-float conversion. The Fibonacci/golden-ratio sample also selects the
saturating `I32::from_f64` result overload explicitly; its remaining integer
cast only erases that same-carrier policy before the process-exit boundary.

### Current realization

Executable `Trapping` and `Saturating` adapters live beside `FloatSemantics`.
`Trapping` judges the semantic result alone; `Saturating` clamps only
finite-operand magnitude overflow and excludes division by signed zero. The
selected adapter and exact operation/format provider identity survive checked
uses through both interpreter and native lowering.

All native target families select exact F32/F64 plans for primitive arithmetic,
comparison, classification, conversion, square root, minimum/maximum, negate,
and multiply-then-add. AArch64 additionally has fused and directed-rounding
realizations. Generic x86-64 remains SSE2-baseline. The explicit
feature-qualified x86 FMA carrier supports source-free custody and final-image
replay, and ordinary exact-target build selection retains that carrier on the
checked compilation. Linux and Windows target sources now select nearest-FMA
plans, and each actual checked use is associated with the exact admitted
provider by full-plan/profile/slot replay. Compiler/native lowering that
consumes this association is still pending.
Multiply-then-add and FMA stay distinct through lowering and result-policy
adaptation.

Generated call/return and foreign callback frames preserve the complete
MXCSR/FPCR control state and install Omega's canonical controls. Returning
foreign calls use the same conservative envelope; direct syscalls do not.
Provider-plan identity, semantic edge cases, interpreter/native agreement, and
cross-target builds are retained as executable evidence rather than copied here
as per-cohort hashes. Remaining provider coverage is tracked in `TASKS.md`.

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
  `Wrapping` = COMPILE ERROR (no modular reading of a float — the fixed-domain
  cast rule generalized; the `range-constraints-require-exact-domain`
  lying-declaration precedent).
- The two axes compose freely (`f32 [0.0..=1.0]::Trapping`): ranges are
  window facts and windows are policy-independent, so no non-Exact-domain lie
  arises (unlike ints, where the range's enforcement mechanism WAS the
  Exact machinery).

## 3. Literals and const-eval: exact Rat, round once

A decimal literal is a rational, exactly. Pipeline: parse -> exact Rat ->
compile-time arithmetic in Rat -> round ONCE at the landing site to the
landing type's format. After landing, every operation evaluates through its
format's executable `FloatSemantics` function, including undefined finite
arithmetic and special values. Constants are unitless until a site requests a
type: deferred typing resolves once at the requesting site, and arithmetic on
the anonymous value is exact.
Conversion vs reinterpretation stays two mechanisms: the value-invariant
mint (`1/3` renders differently per format) is this pipeline; the
bits-invariant read is the recast (`&self.bits as &f32`), footprint-checked,
separate.

Semantic results therefore match target execution by construction. Exact raw
NaN bits require a stronger representation refinement: a build-time raw-bit
observation of a computed possibly-NaN result must prove non-NaN, canonicalize,
or select a realization publishing exact NaN-bit behavior. Runtime recast may
always inspect the bits actually present but promises no cross-target or
cross-build identity.

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
- Total order = a NAMED CONFORMANCE (ch14 machinery, designed for exactly
  this): `sort_by<F64::TotalOrder>` — IEEE totalOrder via sign-magnitude
  integer compare. Rust needed a bolted-on method (total_cmp) and a
  no-Ord-for-floats scar; the conformance is the honest encoding. Posits
  total-order natively, so their `before` member is a plain integer compare.
- `omega::language::core::float_order` provides
  `F32::TotalOrder` and `F64::TotalOrder` as ordinary complete `Order`
  conformances. Their branchless unsigned-key `before` member is exercised
  through static-machine selection in interpreter/native differential
  execution over
  both NaN signs and payload directions, infinities, and `-0.0 < +0.0`; no
  float-order intrinsic or ambient comparison mode exists.
- NaN payloads: absent from the base semantic contract and never
  proof-observable through `FloatMeaning`. A representation-sensitive consumer
  raises the obligation and must prove non-NaN, canonicalize, or demand a
  realization-specific exact-bit refinement. Runtime recast still reads honest
  bits. `f != f` is demoted to IEEE-binding
  detail; `is_finite` is the portable spelling (posits have NaR and a
  native total order — the idiom never fires there).

## 6. Extensibility ladder (user float types) + the posit future

- Rung 1 (available under today's design): a float type as a LIBRARY —
  encoding-domain carrier + proven software ops with decode-to-meaning
  contracts. Zero compiler cooperation.
- Rung 2: first-class literals/const-eval by supplying a FORMAT RECORD;
  the engine's generic round(format) covers any fixed-precision radix-2
  format free (bf16, both FP8s); tapered precision (posits) needs the
  record vocabulary to grow one notch, once.
- Rung 3: hardware — provider-plan bindings selecting compiler-known lowerings
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
   For `a / b -> f32::Finite`, bare finite operands still require proof that
   `b` is neither signed zero and that the rounded quotient does not overflow.
   Saturating discharges the magnitude-overflow branch but not the nonzero
   divisor obligation. A result-checked Trapping qualification may instead
   trap before returning a non-finite value.
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
4. **Canonical hardware state.** Every checked Omega activation uses one
   canonical semantic floating-control configuration. On x86-64 this excludes
   FTZ/DAZ and fixes nearest-even in the relevant MXCSR control mask; on
   AArch64 it fixes the corresponding FPCR control bits. Sticky status flags are
   not part of the invariant. Directed rounding remains a distinct operation,
   not a control-word mutation. The scheduler therefore need not switch
   semantic modes between Omega activations. A foreign binding must either
   prove preservation or save/restore the relevant control state; callbacks
   re-establish Omega state on entry and restore foreign state on exit.
5. **Trapping is a result adapter.** It checks the semantic result rather than
   unmasking hardware floating exceptions. Unmasking does not implement
   Omega's precise policy and would destroy the canonical-control invariant.
   A target may fuse the adapter only with proof against the same contract.
6. **Admitted realization validation.** Executable semantics make vendor
   hardware claims empirically testable even though they remain admitted.
   Target providers retain differential-suite identity and results covering
   normal/subnormal boundaries, ties, overflow/underflow, signed zero,
   infinities, NaNs, and policy edges. Raw-bit comparison is required only when
   an exact NaN representation refinement is claimed. The generic Linux and
   Windows x86-64 baseline suites each retain 36 exact checked provider-plan
   identities, interpreter observations, the explicit target/build binding,
   and two byte-identical target images under host-independent result
   identities. The Linux suite retains ELF output; the Windows suite retains
   PE/COFF output and replays the DOS/PE signature plus AMD64 machine header.
   Hardware execution is retained separately and only on a matching x86-64
   host. Their corpora deliberately exclude directed and fused operations, so
   build success neither selects generic FMA nor widens target admission.
   Linux AArch64 now retains its own comprehensive 56-plan semantic-edge
   receipt, including nearest and directed arithmetic, nearest and directed
   FMA, and fused-versus-separately-rounded behavior. Its checked/interpreted
   half exits 70; two explicit `linux_arm64` roots produce byte-identical ELF
   images naming `EM_AARCH64`; and native execution is retained only on a
   matching Linux AArch64 host. Together with the existing macOS AArch64 suite,
   the four currently admitted native profiles now have target-specific
   executable evidence. This does not admit FMA on generic x86-64.
7. **Engineering order.** Signed Rat -> `FloatMeaning` -> executable semantic
   functions -> policy adapters -> target conformances -> differential
   validation. Signed Rat belongs to the quotient/Real lane and is a hard F7
   dependency. The current core format-record vocabulary already covers
   fixed-precision radix-2.
