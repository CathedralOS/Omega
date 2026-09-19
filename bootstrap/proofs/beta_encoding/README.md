# Beta encoding theory

[theory/theory.gamma](theory/theory.gamma) emits the source-owned transparent
Beta definitions: byte classification, nibble conversion, fixed-width words,
their little-endian byte lists, checked counter increment, unsigned comparison,
and the complete error-valued encoder over the raw source tree.
It does not emit an assembler, owner-root reconstruction, or whole-source
certificate.

The [complete encoding contract](ACCEPTANCE.md)
fixes the acceptance target: the entire selected Gamma evaluator's raw Beta
source and persisted Alpha tape under the complete error-valued encoder.
Keep artifact-specific definitions and eventual root reconstruction/proof
production here, separate from the [generic checker](../checker/README.md).
Formation and generic proof success do not make a partial or producer-selected
Beta theory authoritative.

## Source organization

The current [source closure](theory/theory.gamma.sources) is ordinary
Gamma, executed by the selected Beta-authored Gamma evaluator. The entrance
orders vocabulary and function definitions; `vocabulary.gamma` owns sorts and
constructors, `definitions.gamma` sequences the lexical, nibble, word, and
counter definitions, and their subordinate files own the individual equations.
`definitions/counters.gamma` orders byte helpers, result selection, carry
stages, and the public successor; `definitions/counters/` owns those pieces
and their fixed template writers.
`definitions/ordering.gamma` orders nibble, byte, and word comparisons;
`definitions/ordering/` owns those equations. `definitions/encoding.gamma`
orders the fifty-one encoder definitions after function 57, and
`definitions/encoding/` owns the individual equations. Shared
administrative-field and template writers live in `encoding/`, not in either
arithmetic component.
The checker receives Gamma-emitted package bytes, not host-generated
definitions. No new checker primitive is introduced.

The packed member closure materializes 3,427 lines / 131,059 bytes, SHA-256
`5a5696ae678d81a6a94ccd184db8f22656bf76371916f18cef1d3bd584f13a6d`.
The manifest itself is 5,536 bytes, SHA-256
`6bbb22c38cf490f8d659968524be1584ed4c426ff3c4f5fd4a07bc1f871fef0d`;
`tools/bootstrap/proofs/sources_env.sh` checks both identities against every
materialization and `tests/bootstrap/proofs-identity.sh` covers the refusals.
A digest is an identity check on the member bytes, not artifact authority for
the theory.

## Constructor vocabulary

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
| Word | 5 | Word 277 takes exactly eight Bytes, least significant first. |
| ByteList | 6 | Nil 278; Cons 279 takes one Byte and one ByteList. |
| WordResult | 7 | Overflow 280; WordValue 281 takes one Word. |
| Ordering | 8 | Less 282, Equal 283, Greater 284. |
| Source | 9 | SEmpty 285; SLeaf 286 takes one Byte; SJoin 287 takes two Sources. |
| Expect | 10 | Ready 288, R 289, X 290, RR 291, RX 292, RRX 293: the operand expectation. |
| Status | 11 | Ok 294, Invalid 295, Exhausted 296: the sticky scan status. |
| State | 12 | State 297 takes Bool comment, ByteList reversed pending, Expect, Word count, Status, Word output limit. |
| Fragment | 13 | FEmpty 298; FChunk 299 takes one ByteList; FJoin 300 takes two Fragments. |
| ScanResult | 14 | ScanResult 301 takes State and Fragment. |
| Admission | 15 | Rejected 302; Exhausted 303; Admitted 304 takes one Word byte count. |
| DState | 16 | Token automaton states 305..351; DZgo takes Word and Byte count, DReg2 two Nibbles, DRd1 one Nibble, DZdone one Word, DDone one TokenClass. |
| TokenClass | 17 | TEmpty 352, TInvalid 353, TDw 354, TRegister 355 takes Byte, TWord 356 takes Word, TAssert 357 takes Word, TMnemonic 358 takes Byte opcode and Expect. |
| EncodeResult | 18 | RInvalid 359; RExhausted 360; RSuccess 361 takes one ByteList. |

Every constructor term is finite. Word has exactly the unsigned 64-bit value
domain: eight independent Byte positions, with no shorter, wider, or sign-tagged
inhabitants. It is not a Gamma integer and no checker arithmetic interprets it.
ByteList is a free finite list, suitable for source and tape without a unary
numeric counter. Raw-byte/word custody still belongs to the future artifact owner.

## Lexical definitions

The first four functions each take one Byte, select argument zero in mode 1,
and have exactly 256 clauses in Byte-constructor order. Their bodies contain
only constructors, never recursion or assumptions.

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

## Nibbles and words

| Function identity | Signature | Defining behavior |
| --- | --- | --- |
| 5..20 | `Nibble -> Byte` | Helper with fixed high nibble 0..15; case on low nibble selects the corresponding Byte constructor. |
| 21 `byte_from_nibbles` | `(Nibble high, Nibble low) -> Byte` | Case on high nibble, then call its earlier helper with the unchanged low nibble. |
| 22 `high_nibble` | `Byte -> Nibble` | Select the high four-bit value by a complete 256-case table. |
| 23 `low_nibble` | `Byte -> Nibble` | Select the low four-bit value by a complete 256-case table. |
| 24 `word_bytes` | `Word -> ByteList` | Preserve all eight Word children in low-to-high list order, ending in Nil. |

Each helper has sixteen constructor-only clauses; function 21 has sixteen
clauses calling strictly earlier helpers. Its high-nibble parent slot is
unbound, and its low-nibble argument remains slot 1. Splitting a byte introduces
no primitive division or remainder: the checker sees only constructor cases.

`word_bytes(Word(b0,b1,b2,b3,b4,b5,b6,b7))` has one clause, producing the exact
list `Cons(b0, Cons(b1, ... Cons(b7, Nil)))`. Its eight variables are the immediate
Word children in slots 1..8. Its templates list those variables first, then Nil,
then eight Cons applications with strictly backward references. This is a total
definition for all 2^64 constructor words, not a customer-specific byte pattern.
It supplies the fixed-width serialization needed by every Beta `x` operand and
`dw`, but not their token parser or complete encoder.

Unfolding `byte_from_nibbles` yields a helper application, not a silently
evaluated byte. A round-trip proof must supply both nibble equalities, congruence
to replace the join arguments, the two defining steps for the join, and their
explicit transitive composition. Likewise `word_bytes` preserves any defined
applications inside its Word argument rather than normalizing them implicitly.

## Checked counters

The encoder needs exact source and output lengths for capacity checks and
address assertions. `word_successor` returns `WordValue(w + 1)` for every Word
below `2^64 - 1`, and `Overflow` at that maximum. This is unsigned arithmetic
over the fixed eight-byte representation, not Gamma's signed comparison or
wrapping integer arithmetic. Crossing the highest signed bit remains a success.

| Function identity | Signature | Defining behavior |
| --- | --- | --- |
| 25 `byte_increment` | `Byte -> Byte` | Complete byte table; 255 becomes zero internally. |
| 26 `byte_is_max` | `Byte -> Bool` | True only at 255. |
| 27 `choose_word_result` | `(Bool, WordResult false, WordResult true) -> WordResult` | Return the selected argument without evaluating either branch. |
| 28..35 | `(Byte b0, ..., Byte b7) -> WordResult` | Carry stages for positions 7 down to 0, respectively. |
| 36 `word_successor` | `Word -> WordResult` | Decompose all eight bytes and begin carry propagation at position zero. |

A carry stage checks whether its byte is maximal. If not, it increments that
byte, preserves every other position, and returns WordValue. Otherwise it passes
zero in that position to the earlier, higher-position helper. Carry beyond
position seven returns Overflow. The eight fixed stages need no recursive word
spine; all function dependencies point backward. Only the private byte helper
wraps: the public result never silently wraps a maximal Word to zero.

Explicit derivations must rewrite the Boolean condition before selecting a
branch, then unfold the selected continuation. Unchosen branches need no
normalization. Overflow here is an ordinary value in the Beta theory, distinct
from the generic checker's resource refusal: neither asserts that an exhausted
checker has proved a result. Checked increment does not by itself establish
complete limit enforcement or source/token traversal.

## Unsigned comparison

`word_compare(left, right)` returns Less, Equal, or Greater over the full
unsigned 64-bit Word domain. Address assertions need Equal; source/output
capacity checks need to distinguish Less from Equal and Greater. The comparison
does not use Gamma's signed ordering, subtract words, or wrap a difference.
The highest bit therefore has ordinary positive unsigned weight.

| Function identity | Signature | Defining behavior |
| --- | --- | --- |
| 37..52 | `Nibble right -> Ordering` | Complete sixteen-case helper with fixed left nibble 0..15. |
| 53 `nibble_compare` | `(Nibble left, Nibble right) -> Ordering` | Case on left; apply its earlier helper to right. |
| 54 `ordering_then` | `(Ordering high, Ordering low) -> Ordering` | Preserve Less or Greater; use low only when high is Equal. |
| 55 `byte_compare` | `(Byte left, Byte right) -> Ordering` | Compare high nibbles first, then low nibbles on equality. |
| 56 `word_compare_right` | `(Byte left0, ..., Byte left7, Word right) -> Ordering` | Decompose right and combine the eight byte comparisons. |
| 57 `word_compare` | `(Word left, Word right) -> Ordering` | Decompose left, then call the right-word helper. |

Nibble helpers enumerate all 256 ordered pairs. Byte comparison reuses the
existing high/low nibble definitions; word comparison examines byte seven down
to byte zero. The first differing position decides the result, regardless of
opposing lower positions. Equal requires every position to agree. Although
Word stores bytes least-significant first for serialization, comparison priority
runs in the opposite direction.

The two word clauses expose each operand's eight fields through ordinary
constructor matching. No nested pattern, integer primitive, or new proof rule
is introduced. Before selecting an `ordering_then` clause, an explicit
derivation must rewrite its high argument to an Ordering constructor. These
definitions supply comparison, not the still-missing encoder state transitions
that enforce the actual source/output limits.

## Encoding and execution boundary

`beta_encoding_theory()` writes one complete `GTH1` section and returns scalar
zero to its caller. It has no parameters and reads no input. There is no
production artifact-admitting `main`; the test entry rejects nonempty input
before emission and returns Gamma's marked application result to publish bytes
without an extra scalar terminator. The exact closure and that entry are pinned
by the [theory gate](../../../tests/gamma/beta-encoding-theory/README.md).

The section has eighteen sorts, 361 constructors, and 108 functions. The
vocabulary and outer fields occupy 4,508 bytes; lexical functions occupy
33,200; sixteen fixed-high helpers occupy 8,640; the public join occupies 800;
the split functions occupy 16,440; and word serialization occupies 348. Counter
byte helpers occupy 16,440 bytes, result selection 92, carry stages 2,896, and
the public successor 188. Ordering adds 8,640 bytes of fixed-left nibble
helpers, 800 for public nibble comparison, 124 for result selection, 224 for
byte comparison, 628 for the right-word helper, and 208 for public word
comparison. The error-valued encoder definitions 58..108 occupy 22,816 bytes,
including the 47-state token automaton and the encode entrypoint. Thus the
exact section is 116,992 bytes. All administrative fields fit u31;
none supplies semantic integer constants or operations. [PROFILE.md](PROFILE.md)
records the current source bounds and scoped measurements, separate from
full-certificate acceptance.

## Error-valued encoder definitions

Functions 58..108 implement the complete encoder over the Source tree: list
reversal and flattening, per-byte and whole-source admission, hexadecimal
token shifting, the 47-state token automaton with end-of-token classification,
comment and separator scanning, operand-expectation dispatch, counted output
emission, and the `encode(Source, source_limit, output_limit)` entrypoint
returning `RInvalid`, `RExhausted`, or `RSuccess` of the flattened tape bytes.
Every clause is ordinary constructor case analysis over the vocabulary above;
all function dependencies point backward, and no clause reads input. The
`definitions/encoding/` files are generated by
`tests/gamma/beta-encoding-theory/emit_gamma.py` from the independently
restated declarations in `tests/gamma/beta-encoding-theory/encoding.py`; the
generated text calls only the checked-in literal emitters, so the emitted
bytes match the independent package exactly.

A complete theory is not a complete certificate: the artifact owner must still
fix the raw Beta source and persisted Alpha tape subjects, reconstruct the
owner-fixed proposition, and produce an explicit proof within the selected
resource profile. The measured feasibility blocker for the integrated route is
recorded in [TASKS_BOOTSTRAP.md](../../../TASKS_BOOTSTRAP.md).

Generic formation checks every declaration and every clause, including unused
rows; explicit derivations check classifications, nibble operations and their
composition, word serialization, checked increment, and unsigned ordering against
independent literal expectations. Exact package identity and those checks complement source
audit, but do not discharge the full Beta root.

## Remaining encoder dependency

The definitions now cover the remaining checked arithmetic: hexadecimal
parsing, token and operand state, the complete mnemonic table, address
assertions, failure values, and exact source/output limits and exhaustion.
Structural recursion consumes an unchanged immediate source tail, using
earlier total helpers for state changes; the only mode-1 self-calls are the
structural recursions in functions 70, 71, 72, 74, 82, and 104.

Equation-level evidence now exists: the theory gate checks 200 encoder
equations across all encoder functions 58..108, including tiny end-to-end
encodes, through the ordinary checker. The generic stepper has also
independently reconstructed the owner-fixed proposition over the complete
selected subject and produced the full untrusted derivation — 3,182,484
rows and 135,451,492 request bytes, 16.1 times the 8,388,608-byte provision it
was measured against — so
the certificate is produced and measured and the provisions to admit it are
now selected, pending the Alpha extent-supply leg. [PROFILE.md](PROFILE.md) records the complete
measurements; the residual routes are the owner-level decisions named in the
cost review. Do not let producer-supplied definitions choose the
meaning of the artifact being accepted.
