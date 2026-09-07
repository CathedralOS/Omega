# Integer goals and certificates

The [verifier](verification.md) reconstructs a fixed question from each operation.
A producer proves it using available premises; it cannot substitute a sufficient
but different question. Representability uses
[mathematical terms](mathematical_values.md#integer-terms).

## Canonical scalar goals

Let `n` be the dividend, `d` the divisor, and `MIN` the carrier minimum.
Conjunction/disjunction ordering below is canonical.

| Goal | Canonical question |
| --- | --- |
| Unsigned nonzero divisor or exact division definedness | `1 <= d`. |
| Signed nonzero divisor | `(d <= -1) OR (1 <= d)`. |
| Signed exact division/remainder definedness | `(d <= -2) OR (1 <= d) OR ((d <= -1) AND (MIN + 1 <= n))`. |
| Signed one-bit nonzero divisor | `d <= -1`. |
| Signed one-bit exact division/remainder definedness | `(d <= -1) AND (0 <= n)`; `-2` and `+1` do not inhabit this carrier. |
| Exact shift count | `0 <= count < value_width`. |
| Exact cast/add/subtract/multiply/left-shift representability | The operation's mathematical expression lies in the destination/result carrier. |

Wrapping and saturating divide/remainder still require a nonzero divisor.
Exact division additionally excludes signed `MIN / -1`.

Closed shift counts reduce to truth or falsehood. Symbolic signed counts retain
both needed bounds in canonical proposition order: the value-leading upper
bound precedes the literal-leading lower bound. Unsigned counts omit the
carrier-implied lower bound.
A count carrier may imply the whole goal. Address carriers and mismatched
operand types reject.

Branch facts retain the selected Boolean/integer predicate polarity. Integer
order complements reverse endpoints; integer disequality retains both possible
orders. Literal adjacency is checked, so unsigned `d != 0` gives `1 <= d`,
while signed nonzero gives the ordered two-sign disjunction. These are branch
consequences, not arithmetic search or alternative operation goals. They obey
successor substitution and all-path premise availability. Integer complement
laws do not apply to IEEE comparisons.

## Elementary certificate rules

Every child proof is checked independently. Typed identities and unchanged
endpoints below are exact, not matched by spelling.

| Rule | Permitted conclusion |
| --- | --- |
| `DisjunctionElimination` | From a checked disjunction and one ordered proof per arm, conclude the same goal in every arm. Each branch adds only its own alternative to ambient assumptions; discharged alternatives are not ambient requirements. |
| `EqualitySymmetry` | Reverse one proved scalar equality, retaining its child citation. |
| `IntegerOrderWeakening` | Integer equality or strict order implies `<=` over the same ordered endpoints. No Boolean-to-integer order or reversal. |
| `IntegerOrderSubstitution` | Replace one endpoint of `<` or `<=` through a proved integer equality in either orientation. Preserve the other endpoint and relation kind. |
| `IntegerOrderDiscreteness` | Replace one literal endpoint of fixed-integer `<=` with its immediate outward neighbor to obtain `<`, such as `1 <= length` to `0 < length`. Check adjacency and carrier bounds; no wrap or address carrier. |
| `IntegerCarrierBound` | Exactly `MIN <= value` or `value <= MAX` for a declared fixed-integer SSA value of the matching signed width. No strict, compound, address, or narrower bound. |
| `IntegerSubtractOrder` | From exact fixed-integer `result = original - decrement` and `0 < decrement`, conclude `result < original`. Match all operand identities/types; wrapping subtraction and nonstrict positivity do not qualify. |

Reflexive integer order can use reflexive equality and weakening. Closed strict
comparisons use the closed-integer primitive; relating an evaluated operand
identity requires explicit equality substitution, not trusted constant folding.

## Ordered endpoint certificates

### IntegerAffineBound

Despite its historical name, this rule checks ordered endpoint transformations,
not just a closed-form affine map. Its child proves the exact starting bound.
Its witness binds root, target, source-ordered definition indices, and an aligned
optional literal-landing index for each definition.

A selected non-chain literal is either embedded with its exact type or established
by a strictly earlier, exact same-carrier equality. Each cited landing enters
premise closure before its definition. Missing, late, unused, ambiguous,
redirected, or mistyped landings reject.

The checker replays exact add, subtract, multiply, divide, remainder, and
left/right shifts in definition order. It independently checks continuity,
carrier/operand identity, count bounds, and endpoint arithmetic. Zero divisors,
signed exceptional division, mixed carriers, non-landed runtime counts,
negative/out-of-width counts, stale/reordered definitions, and target drift reject.
Checked arithmetic failure is rejection, not wrapped witness arithmetic.

For affine maps, positive coefficients preserve bound orientation and negative
coefficients reverse it. A zero coefficient retains the cited orientation and
maps to the constant offset. Mapped endpoints must inhabit the checked carrier.

A `Truth` child is permitted only when the checker derives the total image
from the carrier and literal: multiply by zero, remainder, full-carrier-safe
division, right shift, or zero-count left shift. It is not an assertion that an
arbitrary endpoint is safe.

Direct exact arithmetic forms cite independently proved operand endpoints.
Multiplication uses four ordered endpoints and recomputes the four unbounded
corner products. Correlated forms retain the authored factor-sign and
carrier-endpoint quotient comparison, with an earlier literal landing when the
endpoint is an SSA value. Exact add/subtract may use an independently established
complementary endpoint equation. No form may cite the operation's own future
result equation to establish its safety.

### IntegerCastBound

The witness binds a nonempty, source-ordered, contiguous conversion word to its
root, target, and complete fixed-native carrier sequence. Each exact semantic
equality has canonical result orientation and exact adjacent identity.

Validator-legal partial exact casts and strict widening identity edges are
replayed. The surviving root interval is the intersection of every carrier;
a narrowing/cross-sign cast is not claimed total or lossy. Stale, reversed,
discontinuous, cyclic, unsupported, or redirected words reject.

A checked root-bound child maps the same mathematical endpoint into the final
carrier, with every definition recorded in premise closure. A `Truth` child is
allowed only for an exact endpoint of the independently checked carrier
intersection. Wrong orientation, target, or mapped endpoint rejects.

### Correlated forbidden roots

`IntegerCorrelatedForbiddenRoots` handles exact signed division/remainder with
two nonempty, disjoint, source-ordered affine definition branches sharing one
direct fixed-native signed machine parameter. Exact prior literal landings and
nonzero checked coefficients are required.

The checker replays both branches and independently selects the tightest unary
signature bounds strictly inside the carrier limits, retaining the first equal
bound in ledger order. It binds their exact identities and solves integer-lattice
roots for divisor zero and `-1`. A `-1` root is forbidden only where the
dividend evaluates to `MIN`. The witness boundary must equal the reconstructed
semantic-axiom count; the root must be a verifier-supplied machine parameter.

Only an empty checked forbidden-root set proves the unchanged
`ExactDivisionDefined` goal. Partial safety, nonempty forbidden roots,
branch/order/carrier drift, changed bounds, nonparameter roots, arithmetic failure,
or a forged conclusion reject. Definition/landing premises and requirement-bound
premises remain separately recorded.

## Witnesses are not proofs by themselves

Producer-visible affine, cast, shift, and forbidden-root checkers validate
normalization and identity. Their outputs alone establish no root premise,
machine-parameter custody, surrounding arithmetic, or operation safety.

An exact-shift word retains each direction, count, operation index, and any
strictly earlier count-landing index. Counts satisfy `0 <= count < width`;
operation indices strictly increase. Mixed left/right shifts remain ordered,
not a cumulative-count summary. Their validity does not by itself prove
left-shift representability.

A witness gains proof authority only through its specified certificate rule and
independently checked premises. Producer search frontiers, indexes, candidate
preferences, and module organization are implementation details, not additional
proof rules or changes to the canonical goal.
