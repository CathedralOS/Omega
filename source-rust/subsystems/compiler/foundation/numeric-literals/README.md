# Exact numeric literal records

`EvaluatedConstant` retains an exact encoding or an undetermined NaN in a known format.
`IntegerLiteral` and `FloatLiteral` retain source spellings and exact payloads,
with optional explicit landing formats. Integer magnitudes stay inline when
small. Decimal floats are rounded once by the numeric semantics owner when a
consumer requests a format. Parsing these records activates no declaration or
compiler stage; later phases cannot call the lexer through them.

The source compiler, syntax representation and independent semantic consumers
use these records directly. Generic arithmetic belongs to `bignum`; arithmetic
policies and sealed scalar laws belong to [numeric semantics](../../psi/semantics/numeric/).
