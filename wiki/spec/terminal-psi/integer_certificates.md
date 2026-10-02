# Integer goals and certificates

The [verifier](verification.md) reconstructs a fixed question from each operation.
A producer proves it using available premises; it cannot substitute a sufficient
but different question. Representability uses
[mathematical terms](mathematical_values.md#integer-terms).

## Comparison denotations

Builtin fixed-integer comparisons, including mixed widths and signedness, denote
the [exact mathematical relation](../language/numeric_values.md#integer-equality-and-order)
between the original operands. Reconstruction retains both source carriers,
exact value identities, the selected comparison and branch polarity. It must
not reinterpret a negative operand as unsigned or truncate a wider operand.

Any normalization into narrower operation forms must preserve that denotation
with checked subject correspondence and valid conversion premises; a producer
cannot merely relabel operands to satisfy a same-carrier schema. The individual
certificate rules below retain their declared premise restrictions. Their
same-carrier restrictions do not impose a same-carrier source-language rule.
Mathematical embeddings retain each source's exact range and identity.

A mixed signed-index/unsigned-length upper-bound branch supplies no implicit
nonnegativity fact. Bounds and later arithmetic safety still need their exact
independent premises; optimization cannot obtain them from unsigned promotion.

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

Exact subtraction also admits `addr - u64 -> addr` and same-model
`addr - addr -> u64`. Each operand retains its carrier; equal widths do not
permit a count/address conversion. The canonical mathematical difference must
be nonnegative and fit the result carrier. Address distances retain the upper
count bound because address width need not equal count width. A nonliteral
address subtraction cannot become a fixed-integer wrapping operation.

Wrapping and saturating divide/remainder still require a nonzero divisor.
Exact division additionally excludes signed `MIN / -1`.

Closed shift counts reduce to truth or falsehood. Symbolic signed counts retain
both needed bounds in canonical proposition order: the value-leading upper
bound precedes the literal-leading lower bound. Unsigned counts omit the
carrier-implied lower bound.
A count carrier may imply the whole goal. For these shift goals, address carriers
and mismatched operand types reject.

Branch facts retain the selected Boolean/integer predicate polarity. Integer
order complements reverse endpoints; integer disequality retains both possible
orders. Literal adjacency is checked, so unsigned `d != 0` gives `1 <= d`,
while signed nonzero gives the ordered two-sign disjunction. These are branch
consequences, not arithmetic search or alternative operation goals. They obey
successor substitution and all-path premise availability. Integer complement
laws do not apply to IEEE comparisons.

A strict branch order `left < right` between fixed-integer SSA values of the
same carrier also excludes its endpoints: `left <= MAX - 1` and
`MIN + 1 <= right`. The verifier derives these consequences from the selected
predicate and declared carrier, with checked endpoint arithmetic. They supply
neither a relation for another value nor a fact on a different arrival. Existing
endpoint certificates can then prove a one-step increment or decrement safe.

## Elementary certificate rules

Every child proof is checked independently. Typed identities and unchanged
endpoints below are exact, not matched by spelling.

| Rule | Permitted conclusion |
| --- | --- |
| `DisjunctionElimination` | From a checked disjunction and one ordered proof per arm, conclude the same goal in every arm. Each branch adds only its own alternative to ambient assumptions; discharged alternatives are not ambient requirements. |
| `EqualitySymmetry` | Reverse one proved scalar equality, retaining its child citation. |
| `IntegerOrderWeakening` | Integer equality or strict order implies `<=` over the same ordered endpoints. No Boolean-to-integer order or reversal. |
| `IntegerOrderSubstitution` | Replace one endpoint of `<` or `<=` through a proved integer equality in either orientation. Preserve the other endpoint and relation kind. |
| `IntegerMathOrderTranslation` | From mathematical `a <= b`, conclude exactly `a + t <= b + t` for one well-formed mathematical integer term `t`. The child remains independently checked, all embedded value identities and carriers remain scoped, and no finite arithmetic or overflow authority is introduced. Strict premises are outside this certificate rule. |
| `IntegerStrictOrderTransitivity` | Compose `a < b` with `b <= c`, `a <= b` with `b < c`, or two strict orders to conclude `a < c`. The shared middle and ordered endpoints are exact. Both children and conclusion use the same scalar integer type, including carrier, signedness, and width, or all use mathematical integers; no mixed domains or IEEE relations. |
| `IntegerOrderDiscreteness` | Replace one literal endpoint of fixed-integer `<=` with its immediate outward neighbor to obtain `<`, such as `1 <= length` to `0 < length`. Check adjacency and carrier bounds; no wrap or address carrier. |
| `IntegerCarrierBound` | Exactly `MIN <= value` or `value <= MAX` for a declared fixed-integer SSA value of the matching signed width. Its mathematical form bounds `MathValue` by the exact retained carrier endpoints, including an address model's width; it grants no conversion or storage authority. No strict, compound, or narrower bound. |
| `IntegerSubtractOrder` | From exact fixed-integer `result = original - decrement` and `0 < decrement`, conclude `result < original`. Match all operand identities/types; wrapping subtraction and nonstrict positivity do not qualify. |
| `IntegerAddOrder` | From exact fixed-integer `result = original + increment` and `0 < increment`, conclude `original < result`. Match all operand identities/types; wrapping or saturating addition and nonstrict positivity do not qualify. |
| `IntegerSubtractAntitone` | From exact fixed-integer `smaller = minuend - large`, `larger = minuend - small` and `small < large`, conclude `smaller < larger`. Both differences name one exact minuend term and carrier; each subtrahend matches its order endpoint exactly. |

Reflexive integer order can use reflexive equality and weakening. Closed strict
comparisons use the closed-integer primitive; relating an evaluated operand
identity requires explicit equality substitution, not trusted constant folding.

Closed, same-carrier fixed-integer literal equality and order also have exact
`Truth` or `Falsehood` predicate denotations. This conversion uses the primitive
judgment's literal comparison, including signedness and full carrier width;
it evaluates no open storage observation, arithmetic expression, or IEEE relation.
For example, checked transitivity of `3 <= counter` and `counter < 3` yields
`3 < 3`, whose `PredicateDenotation` is `Falsehood`. Existing conversion of
`Falsehood` to `Falsehood AND goal`, followed by conjunction elimination,
then closes an impossible arrival without pruning that arrival or adding an
integer-contradiction rule. Both bound citations remain dependencies, and a
conditional bound is available only within its checked implication/case scope.

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

Unsigned wrapping remainder also admits the nonzero-divisor remainder step.
Its upper endpoint intersects the proved dividend upper bound with `divisor - 1`;
the unconditional remainder image remains a valid certificate conclusion.
A direct exact remainder expression with an exact literal divisor can use the
same endpoint rule without an invented value definition. Zero divisors reject;
the direct signed form also rejects `-1`, since type validity alone does not
establish the signed-minimum exclusion.

For a runtime unsigned fixed-carrier divisor, the witness may instead root at
that exact divisor and retain the remainder expression or one canonical
`result = remainder(dividend, divisor)` definition. An independently checked
`0 < divisor` or `1 <= divisor` child then establishes `result < divisor`.
Exact, Wrapping, and Saturating unsigned remainder share this bound. The
dividend retains its declared carrier and identity; the cited definition stays
in premise closure. A `Truth` child, an unrelated positive value, or a signed
carrier cannot establish this relational form. Formation of the remainder and
availability of its result remain separate verifier obligations.

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

The root may be an admitted integer expression, not only a named SSA value.
The first cast equation must retain that exact expression and validate all of
its nested identities and types. No cast output may occur inside the root.
This transports independently proved bounds; it does not authorize formation
of a partial operation inside the expression.

A strict or inclusive mathematical comparison may also retain a live opposite
endpoint while the checked word replaces the root with its target. Strictness,
endpoint side, and the entire opposite term stay unchanged. Mixed source
carriers retain their mathematical comparison; transport does not introduce a
common carrier or justify a partial cast's formation. Replay derives this step
from cast identities and order substitution without a per-instance rule axiom.

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
