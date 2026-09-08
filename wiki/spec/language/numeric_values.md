# Numeric values and bounds

## Literal landing and destinations

A suffixed integer or float literal retains its chosen carrier or format through
storage, call arguments, state transitions, and returns. An incompatible
destination rejects rather than erasing the suffix. Anonymous numeric arguments
land at the selected explicit parameter; an implicit receiver does not shift that
pairing. Anonymous fixed-integer expressions retain exact rational intermediates
until landing requires an integral, in-range value.

Decimal literals and wholly anonymous arithmetic denote exact rationals.
Anonymous integer-literal division is rational division, not truncation:
the destination checks or rounds the completed value, never selects its
intermediate operations. Float landing rounds once to the selected format;
integer landing requires an integral in-range result. Mixing an already-typed
operand with an anonymous operand first lands the latter for that typed
operation. This introduces no runtime rational arithmetic.

Landed `f32` and `f64` values do not implicitly change format at scalar
destinations. Named values, fields, indexes, call results, and cast outputs retain
their types. A binary result uses the selected operator's result type, not a
guess from operand formats. Explicit casts remain conversion boundaries.

Builtin remainder requires integer operand meaning before folding or destination
checking. A wholly anonymous numeric tree does not acquire that meaning from a
destination, including when an intermediate division is fractional. Retained
range bounds and contract expressions obey the same formation requirement.
Declared operators and proof-level mathematical terms retain their own rules.

## Arithmetic and comparisons

An operation's selected declaration, operand carriers, and arithmetic policy
determine its meaning. Matching a token is insufficient to use builtin arithmetic
or order laws. A negated guard retains the original operator selection before
deriving a consequence; integer order complements are not IEEE float laws.

Every Exact operation owes its own representability proof in its actual carrier,
including unsigned values beyond a signed analysis window. A safe final result,
mathematical substitution, or unknown analysis endpoint cannot excuse an unsafe
intermediate. Wrapping loop updates need an independent no-wrap proof before
monotonicity can be inferred; the proposed descent cannot supply its own premise.
Runtime signed remainder is not mathematical Euclidean modulo.

Exact, Wrapping, Saturating, and Trapping are distinct operation policies.
Wrapping reduces representable-range overflow modulo the selected carrier;
Saturating clamps it to carrier bounds; Trapping uses the primitive's exact
crash predicate. An overflow policy does not invent a result for undefined
operands such as an integer zero divisor. Any number of value qualifications
may compose, but at most one arithmetic policy governs an operation.

### Shift counts

| Policy | Count law |
| --- | --- |
| Exact | Prove `0 <= count < value_width` before formation. |
| Wrapping | Reduce signed or unsigned count by Euclidean modulo `value_width`. |
| Saturating | Prove `0 <= count < value_width`; saturation defines value overflow, not invalid counts. |
| Trapping | An out-of-range count is an executable trap condition. |

Exact left-shift result representability is a separate obligation. Hardware
masking cannot discharge the Exact count proof or silently change its meaning.

Integer bitwise AND, OR, XOR, and complement preserve their signed/unsigned
carrier. They do not acquire Boolean bounds. Bitwise operations themselves add
no overflow-policy requirement; nested and surrounding arithmetic still owes
its obligations. Boolean negation likewise does not skip validation of operands.
Evaluating interval endpoints alone is not a sound bound for AND, OR, or XOR.

Total proof-term formation, `embed`, policy erasure, and FloatMeaning are owned by
[mathematical proof contracts](../proofs/contracts.md#mathematical-values-and-quantification).
Portable operation questions and checked certificate rules are owned by
[integer certificates](../terminal-psi/integer_certificates.md).

## Floating formats and operations

`f32` permanently means IEEE binary32 and `f64` IEEE binary64 on every target
providing them. A new format requires a distinct carrier identity, never a
reinterpretation of an existing name. Formats are semantic data, independent
of their hardware/software realization:

| Core format | Radix | Precision | Normal exponent range | Minimum subnormal exponent |
| --- | --- | --- | --- | --- |
| `FloatFormat::BINARY32` | 2 | 24 | -126 through 127 | -149 |
| `FloatFormat::BINARY64` | 2 | 53 | -1022 through 1023 | -1074 |

Both records include signed zeros, subnormals, infinities, NaNs, and ordinary
round-to-nearest-ties-to-even. Format records name the exponent of the leading
radix digit for normal values and the least subnormal exponent.

Finite nonzero values denote exact signed rationals; operations compute their
exact meaning and round through the selected format, with defined special-value
rules. Core `FloatSemantics` is the common executable definition for proof,
semantic evaluation, folding, interpretation, software realization, and target
validation. Host `f64` arithmetic is not an alternative definition for `f32`.

Primitive spellings use ordinary operand-driven resolution over the complete
static carrier/domain tuple, selecting the exact requirement and target
satisfier/plan. There is no float-only resolver, runtime operation tag,
`FloatOperations<F>` bundle, dynamic dispatch table, or ambient mode. Arithmetic,
comparison, classification, conversion, directed rounding, and fused/unfused
operations have separate normalized requirement identities.

```text
multiply_then_add(a, b, c) = round(round(a * b) + c)
fused_multiply_add(a, b, c) = round(a * b + c)
```

The compiler never contracts the former into the latter. Ordinary operations
round nearest-even; directed modes are separately named requirements. There is
no ambient fast-math flag or build option. A future relaxed operation would
need an explicit per-operation contract; an either-rounding `muladd` remains
deferred without a customer.

### Meaning, comparisons, and representation

`Float::meaning32` and `Float::meaning64` project to proof-only `FloatMeaning`:
finite nonzero `Rat::NonZero`, signed zero, signed infinity, or payloadless NaN.
Subnormals are ordinary nonzero rationals. Rational zero is excluded from the
finite case; infinities/NaNs have no rational embedding. The sum has no runtime
ABI. [FloatMeaning identity and equality](../terminal-psi/mathematical_values.md#floatmeaning)
are structural proof equality, distinct from IEEE runtime comparison.

Under IEEE comparison, ordered relations are false on NaN, `==` is false,
and `!=` is true. Signed zeros compare equal. Integer order complements and
unconditional self-comparison folding therefore do not apply. `is_finite` is
the portable wellness query; `x != x` is an IEEE-specific NaN test.

`minimum(a, b)` returns `a` only when `a < b`; `maximum(a, b)` returns `a`
only when `a > b`. Both return the second operand on unordered or equal inputs,
including equal signed zeros. NaN behavior is operand-order-dependent, not
the IEEE-2019 NaN-propagating `minimum` or a non-NaN-wins rule.

Sorting/keying selects the named `F32::TotalOrder` or `F64::TotalOrder` in
`omega::language::core::float_order`. These implement IEEE totalOrder over
honest representation bits, including NaN signs/payload ordering and
`-0.0 < +0.0`, without changing arithmetic comparison.

NaN payloads are absent only from proof meaning, not from runtime storage.
A runtime [recast](../layouts/recasts.md) reads actual bits but promises no
cross-build/target identity. A compile-time raw-bit observation of a computed
possibly-NaN value requires non-NaN proof, canonicalization, or a selected exact
NaN-bit refinement. [Constant materialization](constants.md#materialization)
enforces this separately from proof-only use.

### Value qualifications and policy adapters

Value qualifications such as `Finite` and float ranges are invariant-window
facts, not runtime metadata or arithmetic modes. `Finite` excludes NaN and both
infinities; a finite float range implies it. Value facts may compose with one
policy; range windows are not restricted to the quiet default.

| Float policy | Meaning |
| --- | --- |
| Unqualified | Quiet format semantics, including non-finite results. |
| Trapping | Trap when the semantic result is NaN or infinity, including propagation from an operand. |
| Saturating | Clamp finite-operand magnitude overflow to the signed largest finite value; do not repair invalid operations or division by signed zero. |
| Wrapping | No candidate; floats have no modular overflow meaning. |

Finite operands alone do not prove finite division: both signed zero divisors
must be excluded and the rounded quotient must not overflow. Underflow to
signed zero is finite. `Finite & Saturating` removes magnitude-overflow work,
not the nonzero-divisor obligation. `0/0`, infinity subtraction, and negative
square root retain their non-finite behavior under saturation. Trapping is a
result adapter, not unmasked hardware floating exceptions; a fused adapter
needs proof of the same contract.

### Destination-owned conversion

Named requirements belong to the destination: for example `F32::from_f64`,
`F32::from_i64`, and `I32::from_f64`. Generic clients name the exact machine
requirement, not a universal `Convert<From, To>` bundle.

Float-format and integer-to-float conversions normally round nearest-even.
Float-to-integer conversion rounds toward zero; a fractional part is lossy,
not itself failure. Its unqualified result requires finite input and a
representable truncated value. `Trapping` traps on failed conversion;
`Saturating` clamps to integer bounds and maps NaN to zero. `Wrapping` has no
float-to-integer candidate. The returned policy remains qualified until
explicit same-carrier `as` erasure selects Exact for later arithmetic.

Selection follows the general [result-domain overload rule](domains.md#result-domain-overloads).
Predicate-only facts do not dispatch; they remain obligations after selecting
the exact result-domain projection. Operator syntax remains operand-directed.

Directed one-step conversions are separately named and cannot consult an
ambient rounding mode. A wrapper composing rounding and conversion must prove
equivalent meaning; it cannot substitute two roundings for a required one-step
conversion. Bare format conversion is total, including infinity. Refinements
such as `Finite` are later predicates, not extra conversion variants.
Same-format qualification belongs to `as`. Failure-returning conversions change
result shape; their public name/carrier remain unsettled with checked-result
arithmetic.

### Target realization and control state

Every checked activation uses canonical floating controls: nearest-even,
gradual underflow, and masked exceptions. On x86 this excludes FTZ/DAZ;
AArch64 has corresponding FPCR requirements. Sticky status flags are not part
of that semantic invariant. Directed operations may internally change controls
only with restoration; they never introduce a source-visible ambient mode.
Schedulers need not switch semantic modes between Omega activations.

Foreign bindings prove preservation or save/restore relevant controls. Callback
entry establishes Omega controls and exit restores foreign controls. A stated
calling-policy predicate alone does not establish execution of those steps.

Target packages select compiler-known instruction lowerings or checked software
machines through explicit satisfiers and retained plans. Format records do not
teach machine encodings; new instructions need the checked instruction catalog.
Hardware meaning remains admitted vendor evidence, while checked software can
be derived against the same executable contract. Differential evidence retains
selected identities and covers normal/subnormal boundaries, ties,
overflow/underflow, signed zeros, infinities, NaNs, and policy edges. Exact NaN
bit comparisons are required only for claimed raw-bit refinements. Compilation
or image reproducibility is not hardware execution evidence.

A library can define an encoded float carrier and checked software operations
without new primitives. First-class literal/evaluation integration requires an
admitted format record; fixed-precision radix-2 records do not already describe
tapered precision. Hardware integration additionally needs selected supported
lowerings. Posit carriers and their extended format vocabulary remain future
work, not permission to rebind `f32`.

The [numeric implementation note](../../../omega-rust/psi/foundation/numerics/README.md)
and [float realization note](../../../omega-rust/omega/compiler/compiler/float_realization.md)
record current support separately from these language contracts.

## Collection bounds

Scalar indexing and range windows have separate bounds obligations. Each nested
array indexing occurrence uses the selected element type's own extent. An
unsupported collection shape supplies no proof.

Collection-relative upper bounds or endpoint ordering alone do not establish
that signed operands are nonnegative. Unknown-length scalar accesses and all
range windows also require lower-bound evidence from enforced types, constants,
or live facts. A nonnegative start and endpoint order may establish a
nonnegative end. Omitted endpoints retain zero/length defaults. All value-dependent
bounds obey [live-fact invalidation](state_contracts.md#mutation-and-subject-identity).

Indexed operator syntax may adapt a fixed-array or slice collection shell to a
shared slice parameter. Element bindings, other operands, domains, ambiguity,
and additional requirements remain exact; named calls do not gain this
conversion. Bounds must follow that selected operator's actual contract, not
another declaration sharing its token. Bounds evidence grants no element-domain,
borrow, mutation, or transfer authority.
