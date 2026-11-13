# Numeric values and bounds

Numeric bounds are propositions established by data-level `requires`, callable
contracts, intentionally named domains, and ordinary flow reasoning. A scalar
carrier does not accept a separate bracketed range annotation. Fields and case
payloads contribute their interval relationships through their owning `requires`;
parameters use `requires`; results use `ensures`; locals use established flow
facts. Explicit nominal qualification follows
[domains](domains.md#declaration-and-membership); internal interval analysis
does not introduce another authored type form.

## Library numeric helpers

`min`, `max`, `abs`, `clamp` and `sqrt` are ordinary standard-library machines,
not language functions or compiler-known source builtins. They belong in
`std`'s math surface, resolve through ordinary imports and declaration identity,
and publish their types, preconditions and guarantees as ordinary contracts.
This settles `executable-numeric-builtins` against the builtin route. A missing
library declaration is a name-resolution failure, not permission to recognize
its spelling in the compiler.

Checking learns result bounds and mathematical meaning from those contracts
and checked bodies, not name-specific proof rules or source-call expansions.
The standard library must make signed-minimum `abs`, inverted clamp bounds,
floating NaNs and signed zeros, and square-root domain and rounding behavior
explicit in its contracts. Existing implementations are not definitions of
unspecified behavior; in particular `abs(x)` is not universally `max(x, 0 - x)`.

Ordinary numeric operators and their admitted target realizations remain
available to implement library code. An optional optimization may realize a
checked operation with a suitable instruction, but does not make any of these
five names intrinsic or replace the ordinary call path. No reserved-name,
qualified-name or implicit-import compatibility route preserves the rejected
builtins. Library placement does not authorize a dependency from `core` to `std`.

## Core integer and float APIs

The owner-ratified `core-integer-float-apis` decision (2026-10-27) places carrier
queries and floating representation utilities in the ordinary core modules
`omega::language::core::integer` and `omega::language::core::float`, not a
catch-all limits module or an automatically imported primitive-type namespace.
Examples use the module-qualified names `integer::max<u32>()` and
`float::max<f32>()`. These zero-value-argument carrier queries are distinct from
two-value comparison helpers such as `max(left, right)` above.

The APIs are ordinary checked library machines. They may derive answers from
authoritative semantic carrier/format metadata or use ordinary exact type-equality
refinement to return carrier-specific values. The compiler supports the general
type/refinement, evaluation, representation and contract machinery; it does not
recognize API names, replace calls with unproved facts, or search for implicit
providers. Physical layout size is not semantic integer width. `u32::MAX`,
`i32::Maximum`, and similar spellings are not aliases, keywords, or a supported
primitive-member lookup route.

Extrema and width queries apply to admitted unqualified fixed integer carriers
at every positive static width, and format queries to the permanent `f32`/`f64`
formats. Unsupported types reject. They do not solve the
maximum of an arbitrary domain, mint qualifications, or assign extrema to
unbounded integers. Carrier-family classification below is a separate total
query, not an extrema query or one compiler-recognized name per width.

The integer query surface is supplied by ordinary checked declarations in
`core::integer`, including exact carrier refinement and static evaluation.
[The endpoint fact](../../../samples/cli/facts/core_integer_endpoint/README.md)
records its transport witnesses. The floating contracts below are ratified
requirements.

### Integer carrier queries

| Core call | Result | Meaning |
| --- | --- | --- |
| `integer::min<T>()` | `T` | Least representable carrier value; zero for unsigned integers. |
| `integer::max<T>()` | `T` | Greatest representable carrier value. |
| `integer::width_bits<T>()` | `u64` | Semantic fixed integer width, not stored field width or an ABI register size. |
| `integer::is_signed<T>()` | `bool` | Whether the carrier has signed integer meaning. |

For width `w`, unsigned bounds are `0` through `2^w - 1`; signed bounds are
`-2^(w-1)` through `2^(w-1) - 1`. Arithmetic policy does not redefine these
carrier bounds. A constrained or routed type does not inherit a constructor
merely because its underlying carrier has those endpoints.

### Floating values and format queries

`float::min<T>()` and `float::max<T>()` return the least and greatest finite
values of `T`, not infinities. In particular, `min` is most-negative finite,
never a smallest-positive value. The following value queries return `T`:

| Core call | `f32` value or exact representation |
| --- | --- |
| `float::min<T>()` | Finite endpoint with bits `0xFF7F_FFFF`. |
| `float::max<T>()` | Finite endpoint with bits `0x7F7F_FFFF`. |
| `float::min_positive_normal<T>()` | `2^-126`, bits `0x0080_0000`. |
| `float::min_positive_subnormal<T>()` | `2^-149`, bits `0x0000_0001`. |
| `float::max_subnormal<T>()` | `2^-126 - 2^-149`, bits `0x007F_FFFF`. |
| `float::epsilon<T>()` | Gap from `1` to its next greater representable value: `2^-23`, bits `0x3400_0000`. Not a universal comparison tolerance. |
| `float::infinity<T>()` | Positive infinity, bits `0x7F80_0000`. |
| `float::negative_infinity<T>()` | Negative infinity, bits `0xFF80_0000`. |
| `float::quiet_nan<T>()` | One canonical positive quiet NaN: `f32` bits `0x7FC0_0000`, `f64` bits `0x7FF8_0000_0000_0000`. |

Format properties are library queries with these exact values. Magnitudes and
counts return `u64`; the three signed exponent queries return `i64`.

| Query in `float` | `f32` | `f64` |
| --- | --- | --- |
| `radix<T>()` | 2 | 2 |
| `precision_bits<T>()` | 24 | 53 |
| `fraction_bits<T>()` | 23 | 52 |
| `exponent_bits<T>()` | 8 | 11 |
| `min_normal_exponent<T>()` | -126 | -1022 |
| `max_normal_exponent<T>()` | 127 | 1023 |
| `min_subnormal_exponent<T>()` | -149 | -1074 |
| `decimal_digits<T>()` | 6 | 15 |
| `roundtrip_decimal_digits<T>()` | 9 | 17 |

Precision includes the implicit leading bit. `decimal_digits` counts guaranteed
significant decimal digits for decimal -> T -> decimal round trips in range;
`roundtrip_decimal_digits` is sufficient for finite T -> decimal -> T round trips
with correctly rounded conversion. These counts do not promise a
particular formatting algorithm or NaN payload encoding.

### Floating representation and classification

The value-level convenience wrappers use ordinary typed overloads:
`float::to_bits(f32) -> u32`, `float::to_bits(f64) -> u64`,
`float::from_bits(u32) -> f32`, and `float::from_bits(u64) -> f64`.
They preserve exact presented representation bits, including signed zeros,
NaN sign, payload, and quiet/signaling state. They perform no numeric conversion,
quieting, canonicalization, or qualification establishment. They are checked
wrappers around [representation views](../layouts/recasts.md) and scalar reads,
not an unchecked transmute or a new general owned-aggregate cast primitive.

`float::classify<T>(value)` returns ordinary case-bearing library data with
`Zero`, `Subnormal`, `Normal`, `Infinity`, and `NaN` alternatives. Every case
retains the sign; `NaN` additionally retains `Quiet` or `Signaling` kind and the
unsigned payload excluding the quiet bit. `f32` uses a `u32` bit/payload carrier
and `f64` a `u64` carrier. Classification and convenience queries
`float::is_nan<T>(value)`, `float::is_signaling_nan<T>(value)`,
`float::is_finite<T>(value)`, and `float::sign<T>(value)` inspect representation
without an arithmetic operation that could quiet a signaling NaN. Sign and kind
are ordinary library cases, not keywords or new primitive species.

Parameterized convenience constructors
`float::quiet_nan<T>(sign, payload)` and
`float::signaling_nan<T>(sign, payload)` construct the requested exact bits.
The payload carrier follows the format above. Only the low 22 bits for `f32`
or 51 bits for `f64` may be present; signaling payloads must be nonzero so the
result is not infinity. Calls prove those requirements or use an ordinary
explicitly failure-returning adapter; constructors never silently truncate.
These are library conveniences, not permanent compiler mechanisms or selectable
policies. Width-indexed unsigned payload carriers may make them unnecessary;
their admission does not automatically deprecate these APIs.
Exact bit composition and its validity/access obligations remain the foundation.

`float::quiet<T>(value)` sets a signaling NaN's quiet bit while preserving sign
and payload, leaving every other representation unchanged.
`float::canonicalize_nan<T>(value)` replaces any NaN with the canonical encoding
above and preserves all non-NaN bits. Both operations are explicit library code;
ordinary transport and the bit wrappers do not insert them.

Exact construction, classification, and transport do not strengthen arithmetic
NaN-result guarantees. Arithmetic may produce any representation allowed by its
existing contract; proof-only `FloatMeaning` still erases NaN payload and sign.
A compile-time observation of computed possibly-NaN bits still requires non-NaN
proof, explicit canonicalization, or an exact-bit refinement. A wrapper cannot
launder that dependency into cross-target reproducibility. Hardware exceptions
remain masked; signaling-state APIs do not promise an ambient hardware trap or
replace Omega's result-based `Trapping` policy.

## Width-indexed integers

Named decision: `width-indexed-integers`, ratified 2026-11-13. `u<N>` and
`i<N>` are unsigned and signed computational integer carriers whose width is
a positive proof-static `u64`. The decimal spellings `u13`, `i257` and the
existing fixed-width names abbreviate those applications: `u32` and `u<32>`
are exactly the same type, not convertible wrappers. Bare `u` and `i` retain
their separate unbounded meaning; omitting the width never requests inference.
Zero widths and runtime width arguments reject. One-bit carriers are admitted:
`u1` contains zero and one; `i1` contains minus one and zero. Neither is `bool`.

The unsigned range is `0 <= value < 2^N`; the signed range is
`-2^(N-1) <= value < 2^(N-1)`, with N-bit two's-complement interpretation for
word operations. Width is numerical meaning, independent of target registers
or physical layout. Generic formation proves positivity under the declaration's
static requirements. Signedness and normalized width form carrier identity
under the ordinary [index rules](generics.md#applications-and-specialization);
proved index compatibility does not merge unrelated nominal carriers.

Literal suffixes admit the concrete width spellings, including `17u13`.
A symbolic width uses an expected `u<N>`/`i<N>` type or an explicit exact
conversion. Literal formation, anonymous arithmetic landing and qualification
retain their existing rules. Small widths confer no exception: a generic body
admitting `i1` cannot assume that positive one inhabits its carrier.

All existing fixed-integer operations apply at N, not at a convenient machine
register width. Exact is still the default, with the existing Wrapping,
Saturating and Trapping policy operators. Intermediates must satisfy their own
contracts; a wider result type does not widen operands retroactively. Wrapping
arithmetic reduces modulo `2^N`. Wrapping shift counts reduce by Euclidean
modulo N, including non-power-of-two widths; hardware register masking is not
an equivalent rule. Unsigned right shifts zero-fill and signed right shifts
sign-extend. Bitwise operations affect exactly N semantic bits, never padding.
The existing zero-divisor, signed `MIN / -1`, remainder and shift-count rules
apply unchanged, including at N = 1.

Comparisons retain exact mathematical-value meaning across integer carriers.
Arithmetic still requires an explicit common typed carrier. Exact numerical
conversion preserves value and proves destination fit, including changes of
signedness; it is not bit reinterpretation. Existing explicitly selected
wrapping, saturating and trapping conversion operations keep their contracts.
Conversion and query libraries generalize over widths rather than requiring
handwritten width-pair tables. `embed` retains the exact source value and range.

### Storage and realization

These are ordinary fixed-size values. Home storage has target/layout-selected
size and alignment and may use several words; it is not implicitly heap-allocated
when N exceeds a register. N does not promise tightly packed array elements or
`size_of` equal to a rounded byte count. Extra physical bits are representation
detail or padding, with no universal zero/sign-extension promise. They cannot
affect numerical equality, arithmetic or value-based hashing. Byte observation
and serialization require their actual layout/encoding contracts; equal storage
sizes alone do not authorize a recast or a reference to a packed field.

Complete integer support includes runtime parameters/results, locals, records,
arrays, copies, comparisons, arithmetic, bit operations and numerical conversions
for every width admitted by the implementation's resource limits. Missing a
single hardware instruction is not a semantic restriction: use correct multiword
lowering or software helpers. Result storage, scratch space and helper calls
enter ordinary resource and stack accounting; helpers cannot introduce hidden
heap allocation, undeclared failure paths or effects. Foreign ABI placement and
atomic access separately require supported, validated realizations. Neither
foreign compatibility nor lock-free access follows from integer formation.

Bound evaluation and realization work before constructing enormous endpoints,
masks, limb arrays or layout products. A width within `u64` does not promise
unlimited compilation or storage resources. Diagnose resource refusal separately
from an unfinished unsupported realization; neither truncates the width, changes
the type's meaning, or counts as complete support. Checker, certificates, Psi
execution and native lowering retain the same width and exact values, including
beyond host scalar limits. Published encodings must preserve them losslessly.

### Integer carrier families

Named decision: `integer-carrier-family-constraints`, ratified 2026-11-13.
Ordinary checked machines in `core::integer` expose these total type queries;
each takes a type argument and returns `bool`:

| Query | Accepted computational carriers |
| --- | --- |
| `is_integer<T>()` | Every `i<N>`, `u<N>`, unbounded `i` and `u`. |
| `is_signed_integer<T>()` | Every `i<N>` and unbounded `i`. |
| `is_unsigned_integer<T>()` | Every `u<N>` and unbounded `u`. |
| `is_fixed_width_integer<T>()` | Every `i<N>` and `u<N>`, not unbounded `i`/`u`. |

Other carriers return false, including `bool`, addresses, floats and proof-only
`Int`/`Nat`. Ordinary alias resolution preserves the underlying carrier's
classification; unrelated nominal data with an integer representation does not
become an integer. Classification of a qualified carrier does not discard its
domains, access, custody or operation-selection obligations.

These queries derive their guarantees from authoritative semantic type metadata.
The compiler checks those guarantees through ordinary refinement machinery,
not function-name recognition, implicit implementation search or one trait
implementation per width. A function returning true supplies no carrier fact
unless its checked contract establishes that fact. The existing `is_signed<T>()`
query remains the fixed-carrier property query; it is not a total type classifier.

Carrier-family facts can constrain ordinary generic machines and domains.
They establish only the operations common to the admitted carriers, with their
ordinary preconditions and explicit operation selections. A symbolic family is
not checked by enumerating its widths. Fixed-width operations and width/extrema
queries require a fixed-width premise; an integer-family premise alone neither
extends unbounded word operations nor supplies a finite maximum. Domain
application checks carrier eligibility before forming the value predicate, as
specified by [generic carrier constraints](generics.md#integer-carrier-constraints).

### Width-family implementation acceptance

Both compiler implementations must preserve this contract through checked
generic use, published/reloaded Psi and native execution on hosted targets:

- One-bit signed and unsigned values exercise literal fit, extrema, overflow,
  division/remainder and conversion boundaries, with rejecting controls.
- A non-byte-width arithmetic customer uses `u13`/`i13` at runtime, including
  sign-extending shifts and modulo-13 wrapping shift counts, not just packing.
- A fixed multiword arithmetic customer uses `u65` and `u257`/`i257` for carries,
  multiplication, division, parameters and results without hidden allocation.
- One generic width body works at narrow and multiword widths without separate
  handwritten source implementations. Concrete shorthand and indexed names have
  identical identity; proved-compatible symbolic widths keep canonical keys.
- Generic nonzero domains accept signed/unsigned carriers as declared, including
  unbounded carriers; unsigned-only membership on a signed carrier rejects as
  an invalid application, not an empty domain. Fake classifiers do not authorize
  integer operations; value-predicate failures remain distinct.
- Layout, padding, packed projection and mutable recast handback controls retain
  value semantics; unsupported foreign ABI or atomic operations reject explicitly.
- Zero/runtime widths, missing generic premises, out-of-range exact conversion,
  malformed wide artifacts and oversized resource requests reject before unsafe
  allocation or truncation. Compare native and Psi results for executable cases.

Genuine unspecified semantic choices discovered during delivery belong in
`OWNER_QUESTIONS.md` with a concrete example and alternatives. Unsupported
implementation is work to complete, not permission to invent different rules.

## Unbounded computational integers

Named decision: `unbounded-computational-integers`, settled 2026-11-11.
`i` admits every signed integer and `u` every nonnegative integer, with literals
such as `7i` and `7u`. They are concrete types, not anonymous numbers, inferred
finite widths, or placeholders for an unselected storage layout. Their value
domains and operations do not change when a realization is chosen. An expected
`i32` result cannot reinterpret `7i` as an `i32` literal.

| Operation | `i` | `u` |
| --- | --- | --- |
| `+`, `*` | Exact integer result, without numerical overflow. | Exact nonnegative integer result, without upper overflow. |
| `-` | Exact signed difference. | Requires right <= left. |
| `/` | Integer quotient, truncating toward zero. | Nonnegative integer quotient. |
| `%` | Remainder corresponding to that quotient. | Corresponding nonnegative remainder. |
| Division or remainder by zero | Must be excluded before formation. | Must be excluded before formation. |

Unary negation on `i` gives the signed result. On `u` it must establish that
the result remains nonnegative; convert to `i` before requesting a negative
result. A cast after an invalid subtraction or negation cannot repair it.

Core computational arithmetic requires the same typed carrier on both sides;
an anonymous operand can land in that carrier. Thus `7i + 2` has type `i`,
while `7i + 2u` and `7i32 + 2i` require an explicit carrier choice.
Nonintegral anonymous operands cannot land in `i` or `u`. This introduces no
automatic widening or signed/unsigned arithmetic promotion.

### Computational conversions

`as i` and `as u` are exact, value-preserving computational conversions.
Every computational integer can convert to `i`; conversion to `u` requires
nonnegativity. Conversion from `i` or `u` to a bounded carrier proves the
destination's range and ordinary validity obligations. These conversions never
round, truncate, wrap or saturate. Source qualifications and authority retain
the ordinary [cast rules](domains.md#domain-casts).

Converting a bounded value to `i` or `u` removes its finite numerical bound
without changing subsequent integer computation into rational arithmetic.
`embed` instead projects a computational integer into mathematical `Int`,
including unsigned inputs with their nonnegativity facts. It preserves the
value already computed, not the meaning of operator tokens after projection.
Direct embedding of bounded integers needs no intermediate `i` or `u`.
This settlement does not add a general mathematical-to-computational conversion.
In particular, a cast cannot manufacture an executable computation of an
arbitrary mathematical value.

### Realization and word operations

Admitted static evaluation and specifications use the same numerical meaning.
Executable uses additionally need an admitted realization and its resource,
effect, failure, custody and calling contracts. An implementation may use fixed
storage for a computation only with checked correctness for its admitted inputs;
it cannot silently narrow `i` or `u`, wrap a result, or approximate with floats.
Resource exhaustion uses the applicable bounded-evaluation outcome, not a
different numerical type or meaning.

There is no implicit wrap modulus, finite maximum, or backend-selected width
for `i` or `u`. Fixed-width rotations, leading-zero counts, complement, shifts
and policy operators do not automatically extend to them. Unbounded bitwise and
shift operations need separately specified contracts. Runtime big-integer storage,
allocation, copying, cleanup and ABI support remain separate delivery work.
Arbitrary finite widths such as `i200` follow [width-indexed integers](#width-indexed-integers),
not layouts that redefine the value domain of `i`.

## Mathematical division

Named decision: `mathematical-exact-division`, settled 2026-11-11.
Core mathematical `/` on `Nat`, `Int` and `Rat` denotes exact rational division.
Its result is always `Rat`, including when the particular quotient is integral.
Each mathematical integer operand is admitted through its checked exact inclusion
into rational arithmetic. An anonymous operand contributes its completed exact
rational value directly; it is not first forced into `Int` or `Nat`.
The divisor must be nonzero before formation, including for symbolic operands.

Consequently, `embed(7u32) / 2` denotes rational `7/2`, and
`embed(6u32) / 2` denotes rational `3` with the same result type.
`embed(7u32) / 0.5` denotes rational `14`. Neither an expected integer result
nor a proof context selects truncating division. Computational-to-mathematical
crossings remain explicit through `embed`.

This specifies `/`'s operand combinations, not a universal promotion system.
Other mathematical operators, parameters, fields, generic applications and
branch joins follow their declared signatures. Unsupported combinations reject;
checking must not invent an inclusion or retry a different arithmetic meaning.
An already typed `Rat` retains its type through folding and needs explicit
value-preserving narrowing before an integer-only use, even when known integral.
Such narrowing proves integrality, and nonnegativity for `Nat`; its focused
public API and broader mathematical conversions are separate from this decision.

The rational interface must have one source-visible `Rat` type, exact denotation,
checked integer inclusions and stable result types. This does not select a
primitive rational carrier, normalize a library representation by compiler fiat,
or approve a full constructor/accessor API. Representation invariants, quotient
versus canonical-record design and correspondence with the existing library
`Rat` require their own checked account. There is no implicit runtime rational
materialization or unchecked bridge to the library's `IntPair` representation.

## Literal landing and destinations

A suffixed integer or float literal retains its chosen carrier or format through
storage, call arguments, terminal transfers, and returns. An incompatible
destination rejects rather than erasing the suffix. Anonymous numeric arguments
land at the selected explicit parameter; an implicit receiver does not shift that
pairing. Anonymous calculations retain exact rational intermediates until their
destination checks the completed value.

Landing selects a carrier and never a domain. A literal bound for a qualified
destination is cast like any other value, as in `5 as +Km`, under
[literals and expected types](domains.md#literals-and-expected-types).

Decimal literals and wholly anonymous arithmetic denote exact rationals.
Anonymous integer-literal division is rational division, not truncation:
the destination checks or rounds the completed value, never selects its
intermediate operations. Float landing rounds once to the selected format;
computational integer landing requires an integral result, destination range
where bounded, and explicit `as` when acknowledging fractional intermediates.
Mathematical integer landing requires final integrality and nonnegativity for
`Nat`, without that acknowledgment. An already typed operand selects its
operation's declared admission rules: computational arithmetic lands an anonymous
peer in its carrier, while mathematical `/` admits its exact rational value.
This introduces no runtime rational arithmetic.

Landed `f32` and `f64` values do not implicitly change format at scalar
destinations. Named values, fields, indexes, call results, and cast outputs retain
their types. A binary result uses the selected operator's result type, not a
guess from operand formats. Explicit casts remain conversion boundaries.

Builtin remainder requires integer operand meaning before folding or destination
checking. A wholly anonymous numeric tree does not acquire that meaning from a
destination, including when an intermediate division is fractional. Retained
range bounds and contract expressions obey the same formation requirement.
Declared operators and proof-level mathematical terms retain their own rules.

Landing chooses a semantic type once, independently of storage realization.
A suffix lands its literal at that
occurrence; a contract comparison obtains its destination from a typed operand
or the owning callable's declared result, not a verifier guess. Parentheses,
constant folding, and evaluation during compilation do not move the landing
boundary. Ordinary machine parameters remain landing boundaries even during
semantic evaluation. No fold may make a landed value anonymous again.

An evaluator's internal exact rational can use an unbounded numerator and
positive denominator, with common factors removed and zero represented by `0/1`.
That does not specify the source-visible `Rat` representation. Nested arithmetic
needs no nested rational carrier or runtime rational evaluator. Anonymous division
by zero has no numeric value, including in proofs or comparisons. No destination
policy supplies one. Nor can a Saturating, Wrapping, or Trapping destination
truncate an anonymous fraction or repair an out-of-range initial value.

### Fractional-intermediate diagnostics

Integer landing of a fractional final value rejects with its exact value and
destination type. The diagnostic explains that typing a computational integer
operand requests integer quotient; typing only the destination does not.

Named decision: `explicit-landing-proof-int-peer`, settled 2026-11-11.
An authored anonymous calculation with a fractional intermediate rejects at an
implicit computational integer landing, including `i` and `u`, even when its
final value is integral and fits any bounded destination.
The diagnostic identifies the fractional origin, exact final value and integer
destination. Suggest `as` around the complete anonymous expression to request
exact mathematical evaluation followed by integer landing, or typing an operand
if integer arithmetic was intended. A destination annotation alone does not
acknowledge the fractional intermediate.

Mathematical `Int` and `Nat` destinations instead accept an anonymous expression
whose final exact value is integral, with nonnegativity additionally required for
`Nat`. Fractional history does not require acknowledgment there. This follows
the destination type wherever mathematical terms are admitted, not the presence
of `ensures`. Already typed rationals still require explicit exact narrowing.

`as T` applied to an anonymous expression, with a computational integer destination `T`,
explicitly lands its complete operand. Evaluate that operand as exact anonymous
arithmetic, then prove final integrality, representability and the target's
ordinary predicate and authority obligations. Fractional intermediates inside
that operand are acknowledged; the cast neither truncates nor retroactively
types them. It cannot repair division by zero, a fractional final value or an
out-of-range result. A carrier cast that also adds domains, such as
`as u32 +Small`, obeys the same rule.

```omega
let implicit: u32 = (5 / 2) * 2;          // Reject: explicit landing required.
let exact: u32 = ((5 / 2) * 2) as u32;    // 5.
let truncated: u32 = (5u32 / 2) * 2;      // 4: typed integer division.
let fractional: u32 = (5 / 2) as u32;    // Reject: 5/2 is not integral.
let overflow: u8 = (256 / 2 * 2) as u8;  // Reject: 256 is out of range.
```

An acknowledgement covers only the cast's operand. `as` binds more tightly
than multiplication: `(5 / 2) * 2 as u32` casts only `2`, so multiplication
requires `5/2` to land as `u32` and rejects. Parenthesize the entire anonymous
calculation when that is what should land. A cast around an enclosing result
does not bypass earlier landing boundaries at machine arguments or typed
operations, nor make already-typed values anonymous again.

The trigger at a computational landing is an actual fractional intermediate, not comparison with a
hypothetical truncating evaluation. Retain its source origin through
simplification: cancellation cannot erase the acknowledgement requirement.
Decimal spelling grants no exemption; `5` and `5.0` denote the same number.
Already-typed integer division and floating landing do not trigger this rule.
Existing exact carrier coercions retain their rules. This is an admission
error, with no warning-suppression escape. Once admitted, the resulting value
has ordinary typed identity: fractional source history is an admission obligation,
not an extra component of integer identity or downstream arithmetic.

## Arithmetic and comparisons

An operation's selected declaration, operand carriers, and arithmetic policy
determine its meaning. Matching a token is insufficient to use builtin arithmetic
or order laws. A negated guard retains the original operator selection before
deriving a consequence; integer order complements are not IEEE float laws.
An early const-argument normalizer retains an application whose operator
selection is unresolved rather than folding it with the token's builtin meaning.
Retention does not establish support for later const evaluation.

Every Exact operation owes its own representability proof in its actual carrier,
including unsigned values beyond a signed analysis window. A safe final result,
mathematical substitution, or unknown analysis endpoint cannot excuse an unsafe
intermediate. Wrapping loop updates need an independent no-wrap proof before
monotonicity can be inferred; the proposed descent cannot supply its own premise.
Runtime signed remainder is not mathematical Euclidean modulo.

There is no implicit widening: arithmetic in `u8` must fit `u8` under Exact;
use an explicit exact conversion to request a wider carrier. Each node produces
its declared-width policy result before its parent consumes it. Wrapping reduces
at that node, Saturating clamps there, and Trapping takes its executable failure
route there. A proved overflow in an already-landed Trapping operation is still
a runtime trap, not an Exact-style compile error; a demanded semantic-evaluation
result separately requires [invocation admission](evaluation.md#invocation-admission).

Adding and removing a policy domain are written [domain casts](domains.md#domain-casts)
and change future operations, not existing payloads. A bare binding cannot
receive a Wrapping value without `as -Wrapping`.
After that removal, Exact operations act on the actual stored
value, not a mathematical pre-wrap result. A machine naming no arithmetic policy
publishes Exact; a caller-selectable policy must occur in its contract. The
generic spelling for that choice remains unsettled. Omission does not choose a
weaker policy, and incompatible policies do not mix implicitly. A
[policy operator](#policy-operators) selects its policy for that one operation
and leaves its operands and result unqualified.

### Integer equality and order

Builtin computational-integer comparisons, including `i` and `u`, use exact
mathematical integer values (`mixed-integer-comparison`). The operands may differ in width, signedness, or
both. `<`, `<=`, `>`, `>=`, `==`, and `!=` apply the corresponding integer
relation to those values and return `bool`. They require no common operand
carrier, implicit cast, or representability of either value in the other
operand's carrier. Equality here is value equality, not type equality.

Each carrier's range is target-independent: `u32` denotes integers from zero
through `2^32 - 1`, and `i32` from `-2^31` through `2^31 - 1`, on every target.
Register width, bit patterns, padding, sign/zero extension, and host-language
promotion rules do not define comparison meaning. In particular, a negative
signed value remains negative when compared with an unsigned value.

| Left operand | Right operand | `<` | `==` | `>` |
| --- | --- | --- | --- | --- |
| `-1` of type `i32` | `10` of type `u64` | true | false | false |
| `10` of type `i32` | `10` of type `u64` | false | true | false |
| `0` of type `i8` | `0` of type `u64` | false | true | false |
| `9223372036854775807` of type `i64` | `18446744073709551615` of type `u64` | true | false | false |

`<=`, `>=`, and `!=` follow the same total integer order and equality. The
comparison itself is total for admitted integer operands; forming either operand
still owes its own operation and validity obligations. Already-typed operands
retain their types; anonymous operands keep the existing literal-landing rules.
This adds neither implicit arithmetic widening nor conversion at assignments,
arguments, or returns. It applies to unqualified operands under the
[operand rule](domains.md#operands): a qualified operand uses a comparison
declared for its exact type or is cast first. It does not override a selected
domain/user operation or grant mixed float, address, or Boolean comparisons.
Arithmetic policies govern computation of the operands, not a reinterpretation
of their resulting values when builtin integer comparison is selected.
This does not implicitly embed a computational operand for comparison with a
mathematical operand; that crossing remains explicit.

Contracts, constant evaluation, checked and Terminal interpretation, optimization,
and native execution must use this same relation. Terminal normalization and
target realization implement it; neither may substitute an unsigned promotion
or reject source operands merely because their integer carriers differ. Operand
evaluation keeps the ordinary order and once-only behavior. Source-independent
verification retains the exact operand subjects, carriers, relation and polarity.

For a signed index, `index < length` establishes only an upper bound: negative
integers also satisfy it when `length` is nonnegative. Indexing separately needs
`0 <= index` under [collection bounds](#collection-bounds).

### Overflow policies

Exact, Wrapping, Saturating, and Trapping are distinct operation policies.
Wrapping reduces representable-range overflow modulo the selected carrier;
Saturating clamps it to carrier bounds; Trapping uses the primitive's exact
crash predicate. An overflow policy does not invent a result for undefined
operands such as an integer zero divisor. A type may carry any number of value
qualifications but at most one arithmetic policy. Which operator a qualified
operand selects follows [exact operator matching](domains.md#semantic-roles-and-operators).

Trapping's language contract is settled — the executable operation owns its
crash site under the primitive's exact predicate. Terminal carries it as one
`TrappingInteger` operation per primitive (add, subtract, multiply, divide,
remainder, shift left, shift right, conversion) that is its own `Trap` site;
the [observation profile](../terminal-psi/observations.md) records that site
in its operation-crash group with the exact primitive denotation, never as a
fabricated edge or boundary route. The trap predicates are the settled
integer-policy catalog rows: result outside the carrier, a zero divisor or
signed `MIN / -1` (including `MIN % -1`), and a shift count outside
`0..width`; a conversion traps when the value is not representable.
Checking, verification, interpretation, and realization preserve the
operation's exact denotation, cause, and path-conditioned site.

### Policy operators

Named decision: `operation-arithmetic-policy`, settled 2026-11-09. A policy can
be chosen at one operation instead of on a binding's type. Each integer
arithmetic and shift operator has three suffixed forms, and the bare operator
stays Exact:

- Wrapping: `+%`, `-%`, `*%`, `/%`, `%%`, `<<%`, `>>%`
- Saturating: `+|`, `-|`, `*|`, `/|`, `%|`, `<<|`, `>>|`
- Trapping: `+!`, `-!`, `*!`, `/!`, `%!`, `<<!`, `>>!`

```omega
let next: u64 = cursor +! width;   // traps when the sum leaves u64
let mixed: u32 = hash *% 31;       // reduces modulo 2^32
let raised: u8 = level +| step;    // clamps to 0..=255
```

A suffixed operator applies its policy's rule for that primitive at that node:
the overflow rule above, the [quotient and remainder](#integer-quotient-and-remainder)
rule and the [shift-count](#shift-counts) rule. It has no other meaning, and no
binding's qualification selects or changes it. `a +% b` on two `u32` values is
exactly what `+` means for two `u32 in Wrapping` operands, and likewise for
every form.

Both operands have the same fixed-integer carrier under the Exact operator's
operand rules; a shift's count keeps its own. At least one operand must already
be integer-typed and the other lands to that carrier. A wholly anonymous tree
rejects, because it has no carrier to wrap, clamp or trap at. Both operands are
unqualified, as for every builtin operator under the
[operand rule](domains.md#operands). A suffixed operator neither consults nor
strips a domain, so an operand carrying any domain is first cast, as in
`(distance as -Km) +% step`. The
result is the bare carrier with no policy qualification, so the next operation
is Exact unless it names a policy itself.

A `%` or `|` form is total over its overflow case and adds no crash site. Like
its policy, it does not license a zero divisor, and a `|` shift still owes the
Exact count proof.

A `!` form is a Trapping operation. It is its own `Trap` site under the
primitive's predicate above, including a zero divisor and an out-of-range shift
count, and it contributes that route to its machine's
[crash contract](effects.md#guarded-crashes) like any other Trapping operation.
Path facts that disprove the predicate remove the route. On normal return the
result equals the operation-specific mathematical denotation of the unsuffixed
primitive, and that equality is an established fact about the result. Integer
division uses truncating quotient, not mathematical rational `/`; see the
[proof correspondence](../proofs/contracts.md#total-arithmetic).

The suffixed forms are compiler-owned integer primitives. A `machine`
declaration cannot bind one, floats have no suffixed forms, and no
compound-assignment spelling is introduced.
[Operator declarations](expressions.md#operator-declarations) owns their lexical
rules.

#### Policy domains

`Wrapping`, `Saturating` and `Trapping` remain qualifications for a value whose
every operation should use one policy. They are `omega::core` domains, not
separately specified arithmetic. For each fixed integer carrier, core declares
each unsuffixed arithmetic operator for two operands carrying the domain and
for one such operand with a plain operand of the same carrier, so
`counter + 1` needs no cast. The operator is the corresponding suffixed
operation on the operands with the domain removed, and its result carries the
same domain. Core declares the comparisons for the same operand shapes; they
compare the stored values and return `bool`. `Trapping`'s operators publish
`crashes Trap`. Their role and the float policy rows are unchanged.

These are ordinary declarations under
[exact operator matching](domains.md#semantic-roles-and-operators). An operand
that also carries another domain, such as `u32 in Km & Wrapping`, matches none
of them. A policy domain is added and removed only by a
[domain cast](domains.md#domain-casts), like every other domain.

Use a suffixed operator when one operation needs a policy. A qualified binding
makes every later operation on it use that policy, and `as -Wrapping` removes
the domain before Exact use. A `Trapping` qualification in particular says nothing about the
value it qualifies: a returned Trapping result is the exact value.

Compiler and library support is pending **OPERATION-ARITHMETIC-POLICY** in
[TASKS.md](../../../TASKS.md).

### Integer quotient and remainder

Builtin `%` needs at least one already-integer-typed operand. The other operand
lands to the required integer type; a fractional operand rejects. A destination
annotation, proof context, positive operands, or known zero remainder cannot
type an anonymous `%`. Its diagnostic requests a typed operand using supported
source spelling, not a destination annotation or hypothetical Euclidean helper.
Already-typed incompatible operands retain ordinary resolution rules.

Computational `/` on bounded integers and `i`/`u` denotes truncating quotient.
Mathematical `/` instead follows [exact division](#mathematical-division).
Integer quotient and remainder remain explicitly available on mathematical
integers through `integer::quotient` and `integer::remainder`. Their contracts
use integer operands, require a nonzero divisor, and preserve the rounding
convention below; their checked mathematical API is distinct from runtime
realization. For nonzero divisor `b`, quotient `q` and remainder `r` obey:

```text
a = q*b + r
|r| < |b|
r = 0 or sign(r) = sign(a)
```

Thus integer quotient of `-7` by `2` gives `-3` with remainder `-1`. A negative
divisor changes the quotient sign, not the dividend-sign remainder convention.
Both operations require a nonzero divisor. Exact rational division is not this
`q`; proof algebra retains the selected operand kinds and operation identities.
Mathematical `%` remains integer remainder with this convention: `Nat % Nat`
produces `Nat`, and `Int % Int` produces `Int`. Other typed combinations need
their declared signatures; this does not add general mathematical promotion. A rational
operand is not implicitly narrowed, even if its value is integral. An anonymous
operand with integral final value can land without fractional-history
acknowledgment at a mathematical integer parameter or `%` operand. A wholly
anonymous `%` still needs an integer anchor; an integer parameter of a named
quotient/remainder call supplies its ordinary destination.
The library's constructed `IntPair` is a distinct nominal type, not builtin
`Int`, and these rules grant neither its operators nor a runtime representation
to proof-only values.

Fixed-width integer division has the same truncation convention. Under Exact,
signed `MIN / -1` and `MIN % -1` both reject because the common quotient is
unrepresentable. Wrapping returns quotient `MIN`, Saturating returns `MAX`, and
both return remainder zero. Neither policy licenses a zero divisor. The
mathematical equation does not itself prove the safety of its authored machine
intermediates or replace overflow-policy behavior.

The [policy operators](#policy-operators) follow these rows. `/%` and `%%`
return `MIN` and zero for signed `MIN` by `-1`, and `/|` and `%|` return `MAX`
and zero; all four still owe a nonzero-divisor proof. On an unsigned carrier,
where no quotient is unrepresentable, they therefore equal `/` and `%`. `/!`
and `%!` trap on a zero divisor and on signed `MIN / -1` or `MIN % -1`.

A separately named Euclidean modulo operation remains deferred with no selected
spelling. There is no context-selected second meaning of `%`, or anonymous
Euclidean-remainder warning: the untyped operation rejects.

### Shift counts

These word-operation rows apply to fixed-width carriers. They do not infer a
width or extend shift/bitwise operations to `i` or `u`.

| Policy | Count law |
| --- | --- |
| Exact | Prove `0 <= count < value_width` before formation. |
| Wrapping | Reduce signed or unsigned count by Euclidean modulo `value_width`. |
| Saturating | Prove `0 <= count < value_width`; saturation defines value overflow, not invalid counts. |
| Trapping | An out-of-range count is an executable trap condition. |

Exact left-shift result representability is a separate obligation. Hardware
masking cannot discharge the Exact count proof or silently change its meaning.

The suffixed shifts take the matching row: `<<%` and `>>%` reduce the count,
`<<|` and `>>|` keep the Exact count proof, and `<<!` and `>>!` trap on an
out-of-range count. A right shift cannot overflow its value, so `>>|` means the
same as `>>`; it exists to keep the suffix set uniform.

Fixed-width integer bitwise AND, OR, XOR, and complement preserve their signed/unsigned
carrier and are total at its width; complement includes the full two's-complement
width of a signed carrier. They do not acquire Boolean bounds. Bitwise operations
themselves add no overflow-policy requirement; nested and surrounding arithmetic still owes
its obligations. Boolean negation likewise does not skip validation of operands.
Evaluating interval endpoints alone is not a sound bound for AND, OR, or XOR.

Total proof-term formation, `embed`, policy removal, and FloatMeaning are owned by
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
static carrier/domain tuple, selecting the exact requirement and the selected
target's declared realization. There is no float-only resolver, runtime operation tag,
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

These are result guarantees, not a requirement to execute operations through
runtime `Rat` values. Hardware or checked software must implement the same
selected contract. Selecting an optimization in `build.omg` cannot weaken the
arithmetic facts used by checking; relaxed arithmetic would need a separately
accepted contract visible to checking as well as realization.

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
float-to-integer candidate. The returned value keeps its policy domain until
a cast such as `as -Trapping` removes it; later arithmetic on the bare carrier
is Exact.

Selection follows the general [result-domain overload rule](domains.md#result-domain-overloads).
Named predicate domains in the declared expected result participate in that
exact domain-set key; proved membership and other flow facts do not. Selection
does not establish a predicate. Operator syntax remains operand-directed.
Without an expected result type, the unqualified overload is selected; a failed
finite/range proof diagnostic lists the available qualified overloads.

Directed one-step conversions are separately named and cannot consult an
ambient rounding mode. A wrapper composing rounding and conversion must prove
equivalent meaning; it cannot substitute two roundings for a required one-step
conversion. Bare format conversion is total, including infinity. The standard
conversion roster does not synthesize extra variants for refinements such as
`Finite`; prove and explicitly add that qualification to the ordinary result.
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

Decision `target-realized-float-arithmetic` (2026-11-09): canonical `f32` and
`f64` operations keep their declaration identities and `FloatSemantics`
contracts and are realized by the selected target's declared realization table,
not by Build provider selection. For each canonical operation a target package
declares one entry: a catalog intrinsic, a checked Omega machine satisfying the
operation's contract, or unsupported. The realization plan retains the chosen
entry with its ISA-feature admission and floating-control obligations. A program
using an unsupported operation on its target rejects, naming the operation and
the target constraint. Build programs neither select nor override canonical
float realizations; a program needing a particular algorithm calls a named
machine. Format records do not teach machine encodings; new instructions need
the checked instruction catalog. Hardware meaning remains admitted vendor
evidence, while checked software can be derived against the same executable
contract. Differential evidence retains
selected identities and covers normal/subnormal boundaries, ties,
overflow/underflow, signed zeros, infinities, NaNs, and policy edges. Exact NaN
bit comparisons are required only for claimed raw-bit refinements. Compilation
or image reproducibility is not hardware execution evidence.

Optional ISA features, including x86 AVX+FMA3, are admitted by the target's
realization table and retained in the realization plan. FMA uses the ordinary operation
and instruction pipeline. Native images require no separate FMA fragment,
occurrence, digest or provider-custody record, and emission does not decode or
replay those records. The superseded constant-operand FMA custody route is not
an alternative lowering path. Instruction encoding and execution tests still
check the selected realization, including its feature admission and floating
controls; this ruling removes no arithmetic guarantee. It does not introduce
a loader-facing ISA-feature manifest.

A library can define an encoded float carrier and checked software operations
without new primitives. First-class literal/evaluation integration requires an
admitted format record; fixed-precision radix-2 records do not already describe
tapered precision. Hardware integration additionally needs selected supported
lowerings. Posit carriers and their extended format vocabulary remain future
work, not permission to rebind `f32`.

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

The [core indexing contract](expressions.md#contract-gated-core-operations)
requires ordinary call-contract discharge for every selected requirement.
Bounds strategies establish reusable propositions over live operand identities;
they do not admit indexing independently of its declaration. Fixed and generic
extents, saved selectors, assignment-length facts and normalized range endpoints
use this same proof interface. Successful discharge retains the evidence for
each emitted access occurrence, separately from its loan and access authority.

## Policy realization

Integer `as` coercions preserve denotation under every target policy and require representability.
The named machines of `core/numeric_conversion.omg` provide explicit wrapping,
saturating, and trapping conversions across signed and unsigned fixed carriers.
Boolean-to-integer conversion and Trapping arithmetic and shifts lower
through Terminal Psi. Trapping integer operations retain their operation-level
`Trap` sites; native realization checks each predicate in place and stops
through the same architectural trap as a `Crash` terminator on x86-64 and
AArch64 at every fixed 8/16/32/64-bit carrier.

Trapping float result adapters are checked against the published `Trap`
ceiling at their exact operation site, including nonentry states. Raw-result
finiteness proofs retain the selected operation and original operands.
