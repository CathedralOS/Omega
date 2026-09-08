# Numeric values and bounds

## Literal landing and destinations

A suffixed integer or float literal retains its chosen carrier or format through
storage, call arguments, state transitions, and returns. An incompatible
destination rejects rather than erasing the suffix. Anonymous numeric arguments
land at the selected explicit parameter; an implicit receiver does not shift that
pairing. Anonymous fixed-integer expressions retain exact rational intermediates
until landing requires an integral, in-range value.

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

Integer bitwise AND, OR, XOR, and complement preserve their signed/unsigned
carrier. They do not acquire Boolean bounds. Bitwise operations themselves add
no overflow-policy requirement; nested and surrounding arithmetic still owes
its obligations. Boolean negation likewise does not skip validation of operands.
Evaluating interval endpoints alone is not a sound bound for AND, OR, or XOR.

Total proof-term formation, `embed`, policy erasure, and FloatMeaning are owned by
[mathematical proof contracts](../proofs/contracts.md#mathematical-values-and-quantification).
Portable operation questions and checked certificate rules are owned by
[integer certificates](../terminal-psi/integer_certificates.md).

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
