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
per-target binding tables are the landed compatibility mechanism being
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
former `Binding::Instruction` compatibility carrier is already retired.

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

Landed 2026-07-28: this is ordinary core proof data.
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

The same semantic function is consumed by proof checking, build-time folding,
the interpreter, checked software realizations, and target-validation tools.
This replaces the current folder path that can evaluate landed `f32` operations
through host `f64`: the semantics becomes the folder, so build-time and runtime
cannot acquire independent definitions.

Implementation checkpoint (2026-07-28): the compiler-side shared engine now
decodes binary32/binary64 into this payload-erased meaning, performs exact
rational arithmetic, and rounds through the matching format record. Exact
decimal landing and anonymous-constant landing use it, as do interpreter
add/subtract/multiply/divide, partial comparisons, the settled min/max choice,
and distinct multiply-then-add/fused-multiply-add definitions. The engine also
now defines named classification, correctly rounded square root, float/integer
and format conversions, and explicit directed-rounding variants. The
interpreter consumes the shared square-root and conversion definitions,
including full unsigned-64 bounds and the settled saturating/trapping edges.
Its f64-backed runtime value carrier preserves f32 NaN sign and payload through
typed landing and raw-bit recasts, so the proof projection can erase payloads
without corrupting representation-sensitive consumers such as `F32::TotalOrder`.
Backend guard-constant folding consumes the same functions rather than host
floating arithmetic followed by a width cast.
The compatibility x86-64 lowering uses a sticky-half-then-double sequence for
upper-half `u64` inputs, while AArch64 selects `UCVTF`; both now preserve source
signedness instead of routing every integer through a signed conversion.
The compatibility `Math::fused_multiply_add` value call now routes through the
shared fused definition in the interpreter, while the native libm binding and
the interpreter are pinned by an edge where fused evaluation leaves a positive
`2^-104` residual and multiply-then-add produces zero.
`omega::language::core::float_operations` now publishes the pure
`FloatSemantics` identities and contracted f32/f64 boundary requirements for
primitive arithmetic/comparison spellings, distinct multiply-then-add/FMA,
classification, and directed rounding. Checked operator evidence records the
selected primitive identities while hardcoded instruction selection remains
the bootstrap realization. Named F32/F64 FMA, classification, and directed
rounding calls now use the same unique path-and-arity resolution in validation,
checked flow, and the interpreter. The call boundary checks argument and result
types, unknown names remain rejected, and executable results come from
`FloatSemantics`. One semantic machine now runs both as a fixed-array-length
build-time invocation and as an interpreted runtime call, with f32/f64 twins
for rounding boundaries, subnormal underflow, overflow, signed zero,
infinities, NaN comparison and min/max behavior, classification, square root,
the directed arithmetic families, and fused-versus-unfused results. This
completes F7 rung 1 for the settled operation surface. The legacy `as`
evaluator path is only a compatibility consumer.

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

Rung-2 checkpoint (2026-07-29): executable policy adapters now live beside
`FloatSemantics`. `Trapping` checks the semantic result alone, so propagating a
pre-existing NaN or infinity traps; diagnostics may inspect the operands but
cannot change that verdict. `Saturating` clamps infinity only when finite
operands produced magnitude overflow, with division by signed zero explicitly
excluded. The interpreter consumes these shared adapters. Checked spelled
binary uses of the imported normalized float surface retain the selected
binary32/binary64
`FloatTrappingNonFinite` or `FloatSaturatingOverflowOnly` adapter beside the
operation identity. Named F32/F64 calls now retain the selected requirement and
same adapter in checked named-use evidence. The interpreter applies it to every
float-returning unary, binary, ternary, and directed operation; classification
results carry no float result adapter, and mixed explicit operand policies
reject statically. The adapter now rides state-graph, control-flow, and abstract
value facts, including nested operators. Normalized table instruction selection
consumes that checked evidence, validates its binary32/binary64 format against
the selected width, and rejects contradictory evidence; only compatibility
operations with no checked operator evidence retain type-domain
reconstruction. Native x86-64 and AArch64 guards implement result-only
`Trapping` for spelled and named result operations, so propagated NaN and
infinity trap as the semantic adapter requires. `Saturating` remains the
operand-aware overflow-only adapter. Stage-copy tests and a native sentinel
canary cover the path. This completes rung 2; rung 3 begins with explicit target
satisfiers and selected `ProviderPlan` realization.

First rung-3 checkpoint (2026-07-31): the shared provider carrier can now reify
one exact overloaded boundary-operator signature as its own single-row service
slot, so binary32 and binary64 requirements cannot collide under the common
name. Core supplies explicit f32/f64 satisfiers for all four primitive
arithmetic and six primitive comparison requirements on `windows_x64`,
`linux_x64`, `linux_arm64`, and `macos_arm64`; each selected plan names the
matching compiler-known operation-and-format realization. Selection retention
validates every selected operator plan even when its requirement is unused,
rejects mislabeled intrinsics and absent exact selection, and attaches the
normalized plan identity to checked spelled and named operator evidence. That
identity survives state graph, control flow, and abstract-operation lowering;
instruction selection resolves it back through the retained selected-plan set
and rejects zero, missing, or contradictory evidence. Cross-target canaries pin
all twenty exact slots per target, every used primitive operation family, and a
native pipeline compile. The named-operation cohort adds exact F32/F64
`minimum`, `maximum`, `square_root`, `negate`, `is_nan`, `is_finite`,
`is_infinite`, `is_normal`, `is_subnormal`, and `multiply_then_add` slots on
every native target. Their checked named-use plan
identity authorizes an
execution-only rewrite in both engine pipelines; proof evidence still names
the source boundary requirement. Negate rewrites the same expression root to
multiplication by a landed negative one. `is_nan` rewrites that root to an
unnameable unary compiler builtin rather than duplicating its operand as
`x != x`, retaining exactly-once evaluation and the operand's binary32/binary64
width through nested lowering. NaN operand order, equal signed-zero selection,
exact-square roots, signed-zero/infinity negation, and NaN/non-NaN predicates
in both formats run in interpreter/native canaries; x86-64/AArch64 cross-target
output and exact binding rejection pin the new realization paths.
The four remaining bool-valued classification predicates use the same
unnameable unary path. Interpreter execution delegates to the shared semantic
engine; x86-64 and AArch64 classify signless IEEE bit patterns against the
minimum-normal and infinity boundaries without touching floating-control
state. An ignored static 4/8 metadata slot retains the authored operand format
when a direct bool write folds that operand to an immediate, while the operand
itself still evaluates exactly once. Zero, normal, subnormal, infinity, and NaN
edges execute in both formats; width-lockstep and cross-target canaries pin the
native paths.
Enum-valued `classify` uses format-specific unnameable builtins and packs the
declared `FloatClass` ABI directly: its source-order i32 tag occupies byte zero
and the overlaid `negative: bool` payload occupies byte four. The acceptance
test derives and asserts that eight-byte layout before executing every tag and
both payload signs in the interpreter, native AArch64, and both Linux target
emitters; exact-binding rejection keeps it distinct from bool predicates.
Multiply-then-add rewrites the selected root to an unnameable format-specific
ternary compiler call so its realization identity survives state-local
expression copying. Native lowering retains the three authored operands, emits
a separate multiply followed by add, and applies result policy only after both
roundings. Binary32/binary64 cancellation edges prove native and interpreter
execution remain unfused; a finite-overflow canary pins operand-aware
Saturating behavior.
Nearest-even F32/F64 FMA now has exact provider slots on `linux_arm64` and
`macos_arm64`. Its distinct unnameable ternary compiler call preserves all
three operands and the authored format; the interpreter calls
`FloatSemantics::fused_multiply_add`, while AArch64 emits one scalar `FMADD`
before applying the final result policy. Binary32/binary64 cancellation edges
pin the positive fused residual, and intrinsic-label rejection keeps the FMA
slot distinct from multiply-then-add. The generic x86-64 targets intentionally
remain SSE2-baseline: selecting FMA3 there without a target feature claim would
be unsound, so they await a feature-qualified or checked software provider.
The six directed F32/F64 FMA slots likewise select exact AArch64 satisfiers.
Their unnameable ternary calls preserve all three operands, the interpreter
uses the matching directed `FloatSemantics` identity, and native lowering
balances an FPCR direction change around exactly one scalar `FMADD` before
result-policy adaptation. Half-ULP edges distinguish all three directions and
prove that the following ordinary FMA remains nearest-even.
The reusable checked-software dispatch path now exists for named boundary
operators. A checked machine body with no `via` must prove equality/`&&`
guarantees covering the operator contract under positional parameter
substitution and may not add a stronger requires
premise; its exact one-row `CheckedAdapter` plan is selected and retained on the
named use before both engines redirect execution to the ordinary Omega body.
This is provider infrastructure, not an FMA implementation: the x86 slots stay
unselected until a checked binary32/binary64 algorithm or honest feature-
qualified target provider is present.
Primitive spellings, the twenty-two cross-target named slots above, and eight
AArch64 FMA slots are migrated, not all of rung 3. Fixed generated callable
frames now save the caller's complete MXCSR/FPCR, install Omega's canonical
semantic controls, and restore the caller's value on return. Their composed
footprints retain `ControlState`, which the state validator admits only as
prescribed `CallReturn` mechanics. This covers ordinary generated entry and
the callback entry/exit seam. Returning foreign mechanisms now conservatively
receive an aligned control-state trampoline too: imported and indirect
vtable/table calls save and restore the complete MXCSR/FPCR around the existing
call program, while direct syscalls add no user-space crossing and receive no
envelope. Layout, emission, and relocation planning share that mechanism
classification; a hostile AArch64 canary calls `_fesetround(FE_UPWARD)` and
proves the following half-ULP checked addition still ties nearest-even. An
explicit preservation-proof optimization may later remove redundant envelopes.
The first directed-rounding provider cohorts select exact F32/F64
add/subtract/multiply/divide/square-root-toward-zero/positive/negative slots on
all four native targets. Baseline x86-64 and AArch64 realize each one-step
operation with a compiler-balanced MXCSR/FPCR save, requested-direction
install, scalar operation, and exact restore before result-policy adaptation.
Midpoint native and
interpreter edges distinguish the three meanings and prove following ordinary
arithmetic remains nearest-even. AArch64 also selects the six directed FMA
slots and balances each requested FPCR direction around one scalar `FMADD`.
x86-64 FMA, checked software fallbacks, and admitted-hardware differential
evidence remain. The first retained differential results are
`omega.float.hardware.macos_arm64.directed-add.v1` /
`0xeb87c478c8a1e513` and
`omega.float.hardware.macos_arm64.directed-subtract.v1` /
`0xc014cab348eb363c`, plus
`omega.float.hardware.macos_arm64.directed-multiply.v1` /
`0xec7e7bae35b056cb` and
`omega.float.hardware.macos_arm64.directed-divide.v1` /
`0xb6dc18215e0c4019`, plus
`omega.float.hardware.macos_arm64.directed-square-root.v1` /
`0x8b87625fd5e9f1b7`. Each binds its six exact selected plan identities to
binary32/binary64 rounding-edge cases, the three requested directions,
control-state restoration, interpreter/native outputs, and Linux
x86-64/AArch64 cross-build success. These are five target/family slices, not
evidence for the remaining hardware realizations. Nearest-even FMA separately
retains `omega.float.hardware.macos_arm64.nearest-fma.v1` /
`0xa1b8c9cb16855a61`, binding its two exact plan identities to binary32/binary64
cancellation cases, one fused rounding, interpreter/native outputs, and Linux
AArch64 cross-build success. Multiply-then-add separately retains
`omega.float.hardware.macos_arm64.multiply-then-add.v1` /
`0x8b5fa3afbbf00653`, binding its two exact plan identities to binary32/binary64
cancellation cases, two distinct roundings, binary32 finite-overflow saturation,
interpreter/native outputs, and both Linux cross-builds. The
minimum/maximum/square-root cohort retains
`omega.float.hardware.macos_arm64.minimum-maximum-square-root.v1` /
`0x8b3cf5ec26298fed`, binding its six exact plan identities to both-format NaN
operand order, the settled signed-zero choices, exact square roots,
interpreter/native outputs, and both Linux cross-builds. The negate/`is_nan`
cohort retains `omega.float.hardware.macos_arm64.negate-is-nan.v1` /
`0x57aa3468298305e9`, binding its four exact plan identities to both-format
signed-zero and infinity negation, NaN/infinity/finite predicate separation,
selected-root unary evaluation shape, interpreter/native outputs, and both Linux
cross-builds. The bool-valued classification cohort retains
`omega.float.hardware.macos_arm64.classification-predicates.v1` /
`0xb89ec4b21c43f9a8`, binding its eight exact plan identities to both-format
boundaries between finite/infinite, infinite/NaN, normal/subnormal, and
subnormal/zero, exactly-once unary evaluation shape, interpreter/native outputs,
and both Linux cross-builds. The enum-valued classification cohort retains
`omega.float.hardware.macos_arm64.classify-enum.v1` /
`0xf63a865e9bbb85f2`, binding its two exact plan identities to the eight-byte
source-order `FloatClass` carrier, sign payload at byte four, every tag and
signed payload in both formats, exactly-once unary evaluation shape,
interpreter/native outputs, and both Linux cross-builds. The format-conversion
cohort retains `omega.float.hardware.macos_arm64.format-conversion.v1` /
`0xeb1e22fdac585936`, binding its two exact directional plan identities to the
binary64-to-binary32 halfway and just-above cases, exact widening, infinity
preservation, interpreter/native outputs, and both Linux cross-builds. The
integer-to-float cohort retains
`omega.float.hardware.macos_arm64.integer-to-float.v1` /
`0x279651cb7ccd80ee`, binding all sixteen exact source/destination plan identities
to narrow signed/unsigned extension, binary32/binary64 precision-boundary ties,
maximum unsigned64 conversion, interpreter/native outputs, and both Linux
cross-builds. The float-to-integer cohort retains
`omega.float.hardware.macos_arm64.float-to-integer.v1` /
`0x297cb8ce8d1adc1c`, binding all twenty exact source/destination/domain plan
identities to both-format truncation toward zero across every integer width,
in-range Trapping dispatch, signed/unsigned/NaN saturation, interpreter/native
outputs, and both Linux cross-builds. Directed FMA separately
retains `omega.float.hardware.macos_arm64.directed-fma.v1` /
`0x75be2c4963f3f15a`, binding its six exact plan identities to binary32/binary64
half-ULP cases, all three requested directions, one fused rounding,
control-state restoration, interpreter/native outputs, and Linux AArch64
cross-build success.

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
- The two axes compose freely (`f32 [0.0..=1.0]::Trapping`): ranges are
  window facts and windows are policy-independent, so no Q9-style lie
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
- Total order = a NAMED SATISFIER (ch14 machinery, designed for exactly
  this): `sort_by<F64::TotalOrder>` — IEEE totalOrder via sign-magnitude
  integer compare. Rust needed a bolted-on method (total_cmp) and a
  no-Ord-for-floats scar; the satisfier is the honest encoding. Posits
  total-order natively, so their satisfier is a plain integer compare.
- Landed 2026-07-23: `omega::language::core::float_order` provides
  `F32::TotalOrder` and `F64::TotalOrder` as ordinary `Order::before`
  satisfiers. Their branchless unsigned-key transform is exercised through
  static-machine selection in interpreter/native differential execution over
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
   an exact NaN representation refinement is claimed.
7. **Engineering order.** Signed Rat -> `FloatMeaning` -> executable semantic
   functions -> policy adapters -> target conformances -> differential
   validation. Signed Rat belongs to the quotient/Real lane and is a hard F7
   dependency. The stale bounded-float canaries remain a cleanup rung; the core
   format-record vocabulary v1 already covers fixed-precision radix-2.
