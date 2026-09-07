# Beta encoding theory

[lexical_theory/theory.gamma](lexical_theory/theory.gamma) emits the first
source-owned portion of the transparent Beta definitions: total byte
classification. It does not emit an assembler, complete encoding theory,
owner-root reconstruction, or whole-source certificate.

The [implementation design](../../../wiki/architecture/bootstrap_chain/derivation_calculus.md)
fixes the acceptance target: the entire selected Gamma evaluator's raw Beta
source and persisted Alpha tape under the complete error-valued encoder.
Keep artifact-specific definitions and eventual root reconstruction/proof
production here, separate from the [generic checker](../derivation_checker/README.md).
Formation and generic proof success do not make a partial or producer-selected
Beta theory authoritative.

## Finite lexical definitions

The current [source closure](lexical_theory/theory.gamma.sources) is ordinary
Gamma, executed by the selected Beta-authored Gamma evaluator. The entrance
orders vocabulary and function definitions; `vocabulary.gamma` owns sorts and
constructors, `definitions.gamma` owns ordered complete cases, `definitions/`
owns the two lexical concerns, and `encoding/` writes administrative fields.
There is no host-generated semantic package or new checker primitive.

Byte is a finite free-constructor sort, not a checker integer. Its 256 nullary
constructors denote exactly the 256 byte values. Nibble likewise has sixteen
nullary constructors. This keeps byte classification to one explicit case step
without unary numeric spines, invalid byte inhabitants, or trusted arithmetic.
Constructor row identities only name symbols; the calculus cannot add them or
infer a numeric ordering from them. The future source/tape owner must map raw
bytes to these exact constructors independently of the certificate producer.

| Sort | Identity | Constructors, in exact wire order |
| --- | ---: | --- |
| Byte | 1 | IDs 1..256 denote byte values 0..255. |
| Bool | 2 | False 257, True 258. |
| Nibble | 3 | IDs 259..274 denote nibble values 0..15. |
| HexResult | 4 | NoHex 275; Hex 276 takes one Nibble. |

All sorts have finite inhabitants. The four functions each take one Byte,
select argument zero in mode 1, and have exactly 256 clauses in Byte-constructor
order. Their bodies contain only constructors, never recursion or assumptions.

| Function identity | Result | Definition from the Beta contract |
| ---: | --- | --- |
| 1 `source_byte` | Bool | True for HT, LF, CR, and printable ASCII 32..126. |
| 2 `separator` | Bool | True for HT, LF, CR, space, and comma. |
| 3 `comment_end` | Bool | True for LF and CR. EOF is not a byte. |
| 4 `hex_digit` | HexResult | Hex(0..9) for `0`..`9`, Hex(10..15) for `a`..`f`, otherwise NoHex. |

These are independent properties: a printable uppercase letter is admitted
source but not a hexadecimal digit. A semicolon starts a comment; it is neither
a separator nor a comment ending. The eventual encoder must check the source
envelope even inside comments, handle the semicolon and EOF, and require whole
tokens. Classification alone cannot establish any of those scanning claims.

## Encoding and execution boundary

`beta_lexical_theory()` writes one complete `GTH1` section and returns scalar
zero to its caller. It has no parameters and reads no input. There is no
production artifact-admitting `main`; the test entry rejects nonempty input
before emission and returns Gamma's marked application result to publish bytes
without an extra scalar terminator. The exact closure and that entry are pinned
by the [lexical gate](../../../tests/gamma/beta-lexical-theory/README.md).

The section has four sorts, 276 constructors, and four functions. Every ordinary
clause is eight words including its length. The sixteen successful hexadecimal
clauses are thirteen words each: a nullary Nibble template followed by Hex of
that template. Thus the theory is exactly 36,532 bytes. Function payload lengths
are 2,054 words for each Boolean classifier and 2,134 for `hex_digit`; these are
wire extents, not semantic constants. All fields fit u31.

The emitter uses no pairs; its marked test entry uses one for its outcome.
Its loops over constructors, cases, functions, and four-byte word emission are
tail calls. The pinned test composition has 19 functions, maximum arity three,
eight nested expression-body lists, and seven active bindings per function.
A source call-path audit allows eight contexts and nine frames, including
temporary pending calls during argument preparation. The deepest retained path
is `main -> theory -> definitions -> function -> cases -> constant_clause ->
constant_template -> fields3 -> word -> word_bytes`; the theory-to-definitions,
function-to-cases, and word-to-word_bytes calls are tail calls. Other preparation
paths, including classification while a clause call is pending, stay within
the allowance. Thus at most 63 binding rows and a conservative
`9 * 10 * 8 + 32 = 752` temporary entries suffice. These are source bounds, not
measured runtime peaks or provisions for a future complete encoder/producer.

On macOS arm64, the 4,847-byte test composition emitted the package in 0.570 s.
The 82,264-byte complete truth-table request passed the unmodified checker in
6.728 s, consuming exactly 137,781 work units. It contains 1,315 ground terms
and 1,024 proof rows. With `S=4, C=276, A=1, F=4, W=9132`, the formation work
estimate is 39,021. The generic cumulative allocation bound specializes to
`22363 + 137781 * 96 + 128 = 13249467` pairs, below the selected Gamma arena.
The checker composition plus framing and this request occupies 145,772 bytes.
These scoped observations do not establish full-certificate size or cost.
Windows runtime validation was unavailable; the gate documents its Git Bash
and Python route.

Generic formation checks every declaration and every clause, including unused
rows; explicit unfolding checks the fixed classifications against independent
literal expectations for all byte values. Exact package identity and those
checks complement source audit, but do not discharge the full Beta root.

## Remaining encoder dependency

Extend the same artifact-specific ownership with fixed-width word/counter
operations, source/tape lists, token and operand state, the complete mnemonic
table, address assertions, little-endian emission, failure values, and exact
source/output limits and exhaustion. Structural recursion must consume an
unchanged immediate source tail, using earlier total helpers for state changes.
Then independently reconstruct the complete owner root and produce its explicit
certificate through the selected source-owned chain. Do not rename this lexical
portion into a complete encoder or let producer-supplied definitions choose the
meaning of the artifact being accepted.
