# Terminal canonical encoding and identities

[Portable product](product.md) | [Observations](observations.md)

This is the canonical encoding contract, not yet a complete byte-level decoder
specification. The [documentation index](../../README.md) lists operation subjects.
[Proof values](mathematical_values.md) and [certificate rules](integer_certificates.md)
define semantics; the tables below give the physical layout of every operation,
terminator, scalar-term, proposition, and proof-node form the codec accepts,
plus the catalog, scalar-block-invariant, machine, and ledger rows a receiver
rederives on decode.

## Canonical form

Semantic bytes begin with `PSITERM\0`. They use fixed-width little-endian counts,
stable nonzero identities, full-width integer payloads, and explicit closed-sum
tags. Unordered sets are strictly sorted by stable identity or canonical bytes
and reject duplicates; symmetric terms use the same ordering. Parameters,
operations, jump arguments, and other execution-significant sequences retain
their declared order. Recursive terms have a fixed supported depth bound.

Decode rejects stale markers, unknown tags, invalid identities/values,
noncanonical order/forms, verifier-invalid modules, truncation, and trailing
bytes. Successful decoding re-encodes byte-for-byte; it never normalizes an
alternative spelling. The pre-release contract has no implicit migration:
semantic changes move producer, codec, verifier, interpreter, and lowerers
together. Current golden bytes establish the current format, not historical
acceptance.

Operation variants are closed and typed. Their referenced values must be
available under the operation's definition/dominance rules. Each reconstructs
its logical result and obligations; an encoded proof cannot select them.

Scalar declarations encode a qualification-set identity after their payload
type. Zero denotes the bare set and has no catalog row. The module's scalar
qualification catalog follows the entry identity as four counted tables:

| Catalog table | Row fields |
| --- | --- |
| domain definitions | domain id + semantic domain id + canonical identity string + scalar carrier type |
| normalized sets | `u64` set identity + counted domain identities |
| qualification-change edges | machine id + edge id + `u32` argument ordinal + source value id + destination value id |
| float entry ranges | machine id + parameter value id + minimum IEEE float value + maximum IEEE float value + `bool` maximum-inclusive flag |

Definitions bind their local and semantic identities, canonical theory
identity, and scalar carrier, and are strictly ordered by domain identity.
Sets are nonempty, contain strictly ordered unique domain identities, and have
unique nonzero identities; duplicate sets reject. Qualification-change rows are
ordered strictly by `(machine, edge, argument ordinal)`; float entry ranges by
`(machine, parameter)`, with both bounds carrying the same IEEE format. All
these bytes participate in semantic identity. Canonical encoding does not
itself authorize membership: verification checks the exact arrival and strict
same-carrier addition or subset erasure under the
[scalar qualification rules](calls_and_outcomes.md#scalar-qualifications).

IEEE scalar comparison uses operation tag 65, followed by a one-byte relation
(`Equal`, `NotEqual`, `Less`, `LessOrEqual`, `Greater`, `GreaterOrEqual` as 0–5)
and the left and right value identities. Both operands have the same IEEE
format and the result is Boolean. Unknown relation tags reject. This executable
operation follows FloatSemantics, including unordered NaN comparisons and equal
signed zeros; it does not assert mathematical equality facts. Source selection
and provider realization custody remain separate from these portable semantics.

Case membership uses operation tag 66, followed by the whole source place
identity, an ordered structural path, and the exact structural case identity. Its ordinary operation result is
an unqualified Boolean. Decoding and verification reconstruct readable access,
nominal case ownership, dominance and current ownership; no payload projection
or reusable mutable-storage equation is encoded by this observation.

Record construction uses operation tag 68 followed
by a counted declaration-order field roster. Each field identity is followed by
operand tag 1 (scalar value identity and optional range obligation) or tag 2
(whole structural argument, including its exact access and path). The exact
result type, multiplicity, and producer remain in the ordinary operation result.
Retired literal tag 51 and scalar-only tag 67 reject; their payloads are not
reinterpreted as the current operand roster.

The semantic scalar-block-invariant roster precedes the machine table.

| Scalar block invariant row | Fields |
| --- | --- |
| invariant | machine id + header block id + one canonical scalar proposition + counted arrivals |
| arrival | edge id + obligation id |

Rows are strictly ordered by `(machine, header)` and arrivals strictly by edge
identity. Every actual header arrival must occur exactly once. Predicate and
roster participate in semantic identity; their certificates remain replaceable
flat proof evidence. The reconstructed ledger owner retains machine, header,
and edge independently of evidence. The block-predicate representation
replaces the former dedicated range row; old bytes are rejected rather than
reinterpreted.

Closed reach applications retain an ordered telescope and finite dependency.
A selected generic callback with emitted applications keeps its original schema
identity and commitment separate from the selected public reach contract. Each
consumer records an exact concrete callee, specialization commitment, and ordered
expected argument tuple;
the verifier rejoins them to that callee's retained closed application. Argument
tags distinguish type, const, proposition, and machine selections; machine
arguments retain selected identity and contract commitment without duplicating
the callee's dependency graph. Missing, differently typed, reordered, or changed
tuples reject even if a stale commitment is unchanged. The original source
projection remains producer-trusted without independently checkable openings.
These contract records add no executable call variant or fuel rule.

An unused selected family retains its selected identity, public contract
commitment, reach row, and requirement bound without requiring an executable
application or schema join header. If the same selection has retained
applications elsewhere in the module, all occurrences retain the same schema
header. Absence of that header does not classify a selected contract as nongeneric.

## Field encodings

Row fields are read in the order given. `u8`, `u16`, `u32`, `u64`, `u128`, and
`i128` are fixed-width little-endian; `bool` is a `u8` restricted to 0 or 1.
`id` is a nonzero `u64` stable identity; `count` and `index` are `u32`.
`counted(x)` is a `count` followed by exactly that many `x` items in row
order. `string` is a `count` plus UTF-8 content bounded by the content-identity
byte limit. Any tag value absent from a closed-sum table rejects.

| Field form | Encoding |
| --- | --- |
| optional identity | `u8`: 0 absent; 1 + `id` |
| scalar type | `u8`: 1 Boolean; 2 + integer type; 3 + IEEE float format |
| integer type | `u8` kind (1 signed, 2 unsigned, 3 address-unsigned) + `u16` bit width |
| integer value | `u8`: 1 + `i128`; 2 + `u128` |
| IEEE float value | `u8`: 1 + `u32` bits; 2 + `u64` bits |
| IEEE float format | `u8`: 1 binary32; 2 binary64 |
| IEEE float relation | `u8`: 0 Equal, 1 NotEqual, 2 Less, 3 LessOrEqual, 4 Greater, 5 GreaterOrEqual |
| IEEE float comparison kind | `u8`: 1 Equal, 2 NotEqual; propositions only — a distinct space from the relation byte |
| structural path | counted segments, each `u8`: 1 + `string` field name; 2 + `u64` fixed index; 3 referent |
| canonical path segment | `u8`: 1 + field id; 2 + `u64` fixed index; 3 + case id |
| canonical structural field | root place id + counted canonical path segments |
| scalar field carrier path | counted segments, each `u8` 1 + field id; record carriers only — other segment tags reject |
| structural access | `u8`: 1 owned, 2 shared borrow, 3 mutable borrow, 4 write-only borrow |
| structural multiplicity | `u8`: 1 unrestricted, 2 affine, 3 linear |
| structural argument | place id + structural access + structural path |
| value declaration | value id + scalar type + `u64` qualification-set identity |
| projected qualification | structural path + domain id |
| structural operation result | place id + structural type id + multiplicity + counted domain ids + counted projected qualifications + counted claims (claim id + structural path) |
| structural parameter | place id + `u32` position + `u8` self flag + structural type id + multiplicity + access + counted domain ids + counted projected qualifications |
| scalar term list | counted scalar terms |
| obligation id list | counted obligation ids |
| claim transfer | claim id + `u32` argument index |
| returned claim transfer | callee claim id + caller claim id |
| completion receipt | claim id + `u32` argument index |
| crash routes | counted buckets, each `u8` cause (1 Trap, 2 Abort) + counted guards; each guard `u8`: 0 truth, 1 + proposition |
| successor edge | edge id + target block id + counted value ids + scalar term list + counted structural arguments + counted trivial discards |
| trivial discard | place id; appears only inside a counted roster |
| residual discard | place id + structural path + structural type id |
| affine cleanup action | `u8`: 1 + place id; 2 + residual discard; 3 + place id + structural type id + machine id + optional identity + obligation id list |
| evidence interface | trait string + counted argument strings + counted requirements (declaring-trait string + counted argument strings + requirement string) |

Admission kinds are shared by proof-bundle admission routes and obligation
classes:

<!-- admission-kind-tags -->
| Tag | Admission kind |
| --- | --- |
| 1 | ForeignBoundaryGuarantee |
| 2 | ProviderFact |
| 3 | CheckedAssemblyClaim |

## Scalar terms

A scalar term is a `u8` form tag followed by its fields. The semantic module
and the proof bundle share this grammar byte for byte.

<!-- scalar-term-tags -->
| Tag | Scalar term | Fields after the tag |
| --- | --- | --- |
| 1 | Value | value id + scalar type |
| 2 | Boolean | `bool` |
| 3 | Integer | integer type + integer value |
| 4 | WrappingIntegerAdd | integer type + scalar term + scalar term |
| 5 | SaturatingIntegerAdd | integer type + scalar term + scalar term |
| 6 | WrappingIntegerSubtract | integer type + scalar term + scalar term |
| 7 | SaturatingIntegerSubtract | integer type + scalar term + scalar term |
| 8 | WrappingIntegerMultiply | integer type + scalar term + scalar term |
| 9 | SaturatingIntegerMultiply | integer type + scalar term + scalar term |
| 10 | BooleanNot | scalar term |
| 11 | BooleanEqual | scalar term + scalar term |
| 12 | IntegerEqual | integer type + scalar term + scalar term |
| 13 | IntegerLessThan | integer type + scalar term + scalar term |
| 14 | IntegerLessOrEqual | integer type + scalar term + scalar term |
| 15 | IntegerBitwiseAnd | integer type + scalar term + scalar term |
| 16 | IntegerBitwiseOr | integer type + scalar term + scalar term |
| 17 | IntegerBitwiseXor | integer type + scalar term + scalar term |
| 18 | WrappingIntegerShiftLeft | value integer type + count integer type + scalar term + scalar term |
| 19 | WrappingIntegerShiftRight | value integer type + count integer type + scalar term + scalar term |
| 20 | IntegerBitwiseNot | integer type + scalar term |
| 21 | IntegerWiden | source integer type + target integer type + scalar term |
| 22 | IntegerExactCast | source integer type + target integer type + scalar term |
| 23 | ExactIntegerShiftRight | value integer type + count integer type + scalar term + scalar term |
| 24 | ExactIntegerShiftLeft | value integer type + count integer type + scalar term + scalar term |
| 25 | ExactIntegerAdd | integer type + scalar term + scalar term |
| 26 | ExactIntegerSubtract | integer type + scalar term + scalar term |
| 27 | ExactIntegerMultiply | integer type + scalar term + scalar term |
| 28 | ExactIntegerDivide | integer type + scalar term + scalar term |
| 29 | ExactIntegerRemainder | integer type + scalar term + scalar term |
| 30 | WrappingIntegerDivide | integer type + scalar term + scalar term |
| 31 | WrappingIntegerRemainder | integer type + scalar term + scalar term |
| 32 | SaturatingIntegerDivide | integer type + scalar term + scalar term |
| 33 | SaturatingIntegerRemainder | integer type + scalar term + scalar term |
| 34 | BooleanField | root place id + counted canonical path segments |
| 35 | IntegerField | root place id + counted canonical path segments + integer type |

## Integer math terms

The proof-only unbounded integer syntax, shared byte for byte between the
semantic module and the proof bundle.

<!-- integer-math-term-tags -->
| Tag | Integer math term | Fields after the tag |
| --- | --- | --- |
| 1 | IntegerLiteral | `u8` negative flag + `u128` magnitude |
| 2 | MathValue | integer type + value id |
| 3 | Add | integer math term + integer math term |
| 4 | Subtract | integer math term + integer math term |
| 5 | Multiply | integer math term + integer math term |
| 6 | ShiftLeft | integer math term + integer math term |

## Content terms and places

The conservation algebra and content-place grammar used by proposition 9,
shared byte for byte between the semantic module and the proof bundle.

| Field form | Encoding |
| --- | --- |
| content algebra | `u8` kind (1 IntervalSet, 2 CountedQuantity) + parameter string |
| content structural place | `u8` version (1 entry, 2 current) + root place id + counted segments; each `u8`: 1 + field-name string, 2 + `u64` fixed index, 3 + case-name string |

<!-- content-term-tags -->
| Tag | Content term | Fields after the tag |
| --- | --- | --- |
| 1 | Projection | domain id + `u64` projection report fingerprint + content structural place |
| 2 | Separate | counted content terms |

## Propositions

A proposition is a `u8` form tag followed by its fields. The semantic module
and the proof bundle share this grammar byte for byte.

<!-- proposition-tags -->
| Tag | Proposition | Fields after the tag |
| --- | --- | --- |
| 1 | Truth | — |
| 2 | Falsehood | — |
| 3 | Atom | proposition id |
| 4 | Equal | scalar term + scalar term |
| 5 | LessThan | scalar term + scalar term |
| 6 | LessOrEqual | scalar term + scalar term |
| 7 | Conjunction | counted propositions |
| 8 | Implication | premise proposition + conclusion proposition |
| 9 | ContentConservation | content algebra + content term + content term |
| 10 | Disjunction | counted propositions |
| 11 | IeeeFloatComparison | IEEE float comparison kind + IEEE float format + canonical structural field + canonical structural field |
| 12 | ByteSequenceEqual | canonical structural field + canonical structural field |
| 13 | StructuralCaseMembership | canonical structural field + case id |
| 14 | IntegerMathEqual | integer math term + integer math term |
| 15 | IntegerMathLessThan | integer math term + integer math term |
| 16 | IntegerMathLessOrEqual | integer math term + integer math term |

## Operation rows

A block row is a block id, counted parameter value declarations, counted erased
scalar formal declarations, counted structural parameters, counted operation
rows, and one terminator row.

An operation row is an operation id, a `u8` static-reach flag (1 adds a `u32`
argument binding; 0 records none), a `u8` result tag (0 unit; 1 + value
declaration; 2 + structural operation result), the `u8` operation tag, and
that tag's fields.

<!-- operation-tags -->
| Tag | Operation | Fields after the tag |
| --- | --- | --- |
| 1 | IntegerConstant | integer value |
| 2 | BooleanConstant | `bool` |
| 3 | WrappingIntegerAdd | left value id + right value id |
| 4 | SaturatingIntegerAdd | left value id + right value id |
| 5 | WrappingIntegerSubtract | left value id + right value id |
| 6 | SaturatingIntegerSubtract | left value id + right value id |
| 7 | WrappingIntegerMultiply | left value id + right value id |
| 8 | SaturatingIntegerMultiply | left value id + right value id |
| 9 | BooleanNot | operand value id |
| 10 | BooleanEqual | left value id + right value id |
| 11 | IntegerEqual | left value id + right value id |
| 12 | IntegerLessThan | left value id + right value id |
| 13 | IntegerLessOrEqual | left value id + right value id |
| 14 | IntegerBitwiseAnd | left value id + right value id |
| 15 | IntegerBitwiseOr | left value id + right value id |
| 16 | IntegerBitwiseXor | left value id + right value id |
| 17 | WrappingIntegerShiftLeft | value id + count value id |
| 18 | WrappingIntegerShiftRight | value id + count value id |
| 19 | IntegerBitwiseNot | operand value id |
| 20 | IntegerWiden | operand value id |
| 21 | IntegerExactCast | operand value id + obligation id |
| 22 | ExactIntegerShiftRight | value id + count value id + obligation id |
| 23 | ExactIntegerShiftLeft | value id + count value id + obligation id |
| 24 | ExactIntegerAdd | left value id + right value id + obligation id |
| 25 | ExactIntegerSubtract | left value id + right value id + obligation id |
| 26 | ExactIntegerMultiply | left value id + right value id + obligation id |
| 27 | ExactIntegerDivide | left value id + right value id + obligation id |
| 28 | ExactIntegerRemainder | left value id + right value id + obligation id |
| 29 | WrappingIntegerDivide | left value id + right value id + obligation id |
| 30 | WrappingIntegerRemainder | left value id + right value id + obligation id |
| 31 | SaturatingIntegerDivide | left value id + right value id + obligation id |
| 32 | SaturatingIntegerRemainder | left value id + right value id + obligation id |
| 33 | Call | callee machine id + counted value ids + scalar term list + obligation id list + crash routes |
| 34 | CallUnit | callee machine id + counted value ids + scalar term list + counted structural arguments + counted claim transfers + obligation id list + crash routes |
| 35 | BoundaryCall | boundary machine id + counted value ids + counted structural arguments + counted completion receipts |
| 36 | PortWrite | service id + `u16` port + `u8` value |
| 37 | EstablishTrivialAffineLocal | place id |
| 38 | BooleanStructuralField | source place id + scalar field carrier path + field id |
| 39 | CallStructuralScalar | callee machine id + counted value ids + scalar term list + counted structural arguments + counted claim transfers + obligation id list + crash routes |
| 40 | EstablishByteSequenceLiteral | destination place id + counted bytes |
| 41 | CallStructural | callee machine id + counted structural arguments + counted claim transfers + counted returned claim transfers + obligation id list + crash routes + counted selected evidence bindings |
| 42 | EstablishScalarCase | case id + counted fields (field id + value id + optional obligation id) |
| 43 | WriteOnlyPrimitiveStore | destination place id + value id; root-only form |
| 44 | IeeeFloatConstant | IEEE float value |
| 45 | NearestIeeeFloatFusedMultiplyAdd | left value id + right value id + addend value id |
| 46 | StructuralScalarFieldStore | destination place id + structural path + field id + value id; unchecked form |
| 47 | IntegerStructuralField | source place id + scalar field carrier path + field id |
| 48 | CallDynamicScalar | `u32` descriptor ordinal + obligation id list + crash routes |
| 49 | CallDynamicParameterScalar | `u32` parameter ordinal + `u32` requirement slot + obligation id list + crash routes |
| 50 | CallStructuralWithScalarArguments | callee machine id + counted value ids + scalar term list + counted structural arguments + counted claim transfers + counted returned claim transfers + obligation id list + crash routes |
| 52 | CallDynamicUnit | `u32` descriptor ordinal + obligation id list + crash routes |
| 53 | CallDynamicParameterUnit | `u32` parameter ordinal + `u32` requirement slot + obligation id list + crash routes |
| 54 | StoreDynamicDescriptor | `u32` descriptor ordinal |
| 55 | ByteSequenceLength | source place id |
| 56 | ByteSequenceRead | source place id + index value id + length value id + obligation id |
| 57 | ByteSequenceSubslice | source place id + start value id + end value id + length value id + obligation id |
| 58 | StructuralByteSequenceFieldStore | destination place id + structural path + field id + source place id + length value id + obligation id |
| 59 | StructuralByteSequenceFieldLength | source place id + structural path + field id |
| 60 | StructuralByteSequenceFieldByteStore | destination place id + structural path + field id + index value id + value id + length value id + obligation id |
| 61 | EstablishPrimitiveLocal | value id |
| 62 | PrimitiveScalarRead | source place id; root-only form |
| 63 | ByteSequenceWrite | destination place id + index value id + value id + length value id + obligation id |
| 64 | EstablishScalarArray | counted value ids |
| 65 | IeeeFloatCompare | IEEE float relation + left value id + right value id |
| 66 | StructuralCaseMembership | source place id + structural path + case id |
| 68 | EstablishRecord | counted fields (field id + `u8` operand tag: 1 + value id + optional obligation id; 2 + structural argument) |
| 69 | EstablishReference | structural argument |
| 70 | ReleaseReference | place id |
| 73 | PrimitiveScalarRead | canonical structural field; projected form requires a nonempty path |
| 74 | WriteOnlyPrimitiveStore | canonical structural field + value id; projected form requires a nonempty path |
| 75 | StructuralScalarFieldStore | destination place id + structural path + field id + value id + obligation id; range-checked form |
| 76 | WriteOnlyIndexedPrimitiveStore | canonical structural field + index value id + value id + obligation id |
| 77 | MoveStructuralField | source place id + structural path + field id |
| 78 | StoreStructuralField | destination place id + structural path + field id + structural argument |

Tags 51 (retired literal field row) and 67 (retired scalar-only record
operand) reject; their payloads are not reinterpreted as current forms. Tags
71 and 72 are unassigned. Every other tag absent from the table rejects as an
unknown operation tag. The tag-41 selected evidence binding is: result type
id + result case id + `u32` guarded position + callee obligation id + callee
term id + output field string + callee proposition id + instantiated
proposition id + output evidence term id + result substitution (`u8`: 0
absent; 1 + `u32` argument position + callee result place id + caller result
place id) + validity record (result place id + counted proposition-dependency
place ids + evidence interface + counted interface-dependency place ids) +
`u32` expected use count + counted uses (target machine id + `u32` input
position + target requirement proposition id + target evidence term id +
source evidence term id + instantiated proposition id + target parameter
place id + caller result place id).

## Terminator rows

A terminator row is a `u8` tag followed by its fields.

<!-- terminator-tags -->
| Tag | Terminator | Fields after the tag |
| --- | --- | --- |
| 1 | Jump | edge id + target block id + counted value ids + scalar term list + counted structural arguments + counted trivial discards |
| 10 | Jump | the tag-1 fields + counted residual discards; residual list must be nonempty |
| 2 | Return | edge id + value id + counted affine cleanup actions |
| 3 | Conditional | condition value id + successor edge + successor edge |
| 4 | Crash | edge id + `u8` cause (1 Trap, 2 Abort) + counted propositions + counted claim ids |
| 5 | ReturnUnit | edge id + counted trivial discards |
| 6 | ReturnStructural | edge id + source place id + counted claim ids + counted trivial discards |
| 7 | ReturnUnitPartialAffine | edge id + counted trivial discards + counted residual discards |
| 8 | ReturnUnitNominalAffine | edge id + counted cleanups (place id + structural type id + cleanup machine id + optional receiver place id + obligation id list) |
| 9 | StructuralCase | source place id + counted case successors (edge id + target block id + case id + counted field ids + counted trivial discards) |

A Jump with no residual affine discards uses tag 1; a nonempty residual list
uses tag 10. Both have the same jump semantics. Tag 10 with an empty list is
noncanonical. Each residual retains place, ordered structural path, and exact
subtree type; ownership validation reconstructs the complement rather than
trusting this list.

## Machine rows

Module bytes are `PSITERM\0` + `u16` format marker 101 + `u16` vocabulary
marker 107 + the entry machine id, followed by the module's counted tables in
declaration order and ending with the machine roster. The roster is strictly
ordered by machine id. A machine row is its machine id followed, in order, by
an optional attachment structural type identity, counted parameter value
declarations, counted structural parameters, a ranked-cycle byte, a machine
result row, counted structural place declarations, counted entry claims, the
declared service reach and published service ceiling, a closed reach
application, counted content entry claims, counted content identity
reshuffles, counted content partition compositions, the entry block id,
counted block rows, and the machine contract.

| Machine row part | Fields |
| --- | --- |
| structural place declaration | place id + structural place kind |
| entry claim | claim id + input place id + structural path; strictly ordered by claim id |
| service ceiling | counted service ids; the published ceiling is strictly ordered |
| structural result declaration | place id + structural type id + structural multiplicity + counted domain identities + counted projected qualifications + counted reference result sources |
| reference result source | result structural path + source structural argument; strictly ordered by result path |
| machine contract | contract id + crash routes + counted erased scalar formal declarations + counted requires propositions + counted ensures clauses + counted outcome-specific ensures |
| ensures clause | obligation id + proposition |
| outcome-specific ensure | result type id + result case id + `u32` outcome position + obligation id + proposition + outcome evidence |
| outcome evidence | `u8`: 0 absent; 1 + evidence term id + output field string |
| content entry claim | claim id + content structural place + counted claim content projections |
| content identity reshuffle | claim id + input content structural place + output content structural place + counted claim content projections |
| claim content projection | domain id + `u64` projection report fingerprint + content algebra |
| content conservation | content algebra + content term + content term |
| place substitution | source content structural place + target content structural place |
| content partition composition | producer operation id + `u64` source report fingerprint + counted source place declarations + source content conservation + counted input claim ids + counted place substitutions + derived content conservation; place kind 8 rejects inside the source places |

The ranked-cycle byte is 0 when the machine retains no ranking; otherwise a
ranked-SCC row follows:

<!-- ranked-scc-tags -->
| Tag | Ranked SCC | Fields after the tag |
| --- | --- | --- |
| 2 | Natural | counted natural cycle components |

A natural cycle component is its rank integer type, counted block ranks (block
id + rank value id), and counted rank edges. A rank edge is its edge id,
source and target block ids, successor rank value id, and rank comparison:

<!-- rank-comparison-tags -->
| Tag | Rank comparison | Fields after the tag |
| --- | --- | --- |
| 1 | Preserving | — |
| 2 | Strict | — |

<!-- machine-result-tags -->
| Tag | Machine result | Fields after the tag |
| --- | --- | --- |
| 0 | Unit | — |
| 1 | Scalar | value declaration |
| 2 | Structural | structural result declaration |

<!-- place-kind-tags -->
| Tag | Structural place kind | Fields after the tag |
| --- | --- | --- |
| 1 | Parameter | `u32` position + `bool` self flag |
| 2 | Result | — |
| 3 | TrivialAffineLocal | `u32` declaration ordinal + structural type id; no construction element |
| 4 | ByteSequenceLiteral | `u32` declaration ordinal + structural type id |
| 5 | ProviderAttachment | attachment structural type id + field id + boundary machine id |
| 6 | OperationResult | producer operation id + structural type id |
| 7 | TrivialAffineLocal | `u32` declaration ordinal + structural type id + root structural type id + `u64` construction index |
| 8 | BlockParameter | block id + `u32` position |

Content rows share these closed `u8` spaces. A content structural place is
its version byte + root place id + counted segments.

<!-- content-place-version-tags -->
| Tag | Content place version | Fields after the tag |
| --- | --- | --- |
| 1 | Entry | — |
| 2 | Current | — |

<!-- content-place-segment-tags -->
| Tag | Content place segment | Fields after the tag |
| --- | --- | --- |
| 1 | Field | field name string |
| 2 | FixedIndex | `u64` index |
| 3 | Case | case name string |

<!-- content-algebra-kind-tags -->
| Tag | Content algebra | Fields after the tag |
| --- | --- | --- |
| 1 | IntervalSet | parameter string |
| 2 | CountedQuantity | parameter string |

<!-- content-term-tags -->
| Tag | Content term | Fields after the tag |
| --- | --- | --- |
| 1 | Projection | domain id + `u64` projection report fingerprint + content structural place |
| 2 | Separate | counted content terms; nesting deeper than 256 rejects |

A closed reach application is a `bool` presence flag; when present it carries
the template identity string, the 32-byte template commitment, the 32-byte
specialization commitment, a counted telescope, the fixed service ceiling, a
counted `u32` dependency binder list, and counted call rows. A call row is an
operation id, a `u32` binder, and a `bool` application presence flag followed,
when present, by the call application.

| Closed reach row | Fields |
| --- | --- |
| machine binding | `bool` nominal-requirement presence + requirement string when present + upper-bound service ceiling + selected identity string + 32-byte selected contract commitment + selected-reach service ceiling + optional callee machine identity + `bool` schema presence + schema template identity string and 32-byte schema template commitment when present |
| call application | callee machine id + 32-byte specialization commitment + counted reach arguments |

<!-- reach-parameter-tags -->
| Tag | Telescope parameter | Fields after the tag |
| --- | --- | --- |
| 0 | Type | argument string |
| 1 | Const | argument string |
| 2 | Proposition | argument string |
| 3 | Machine | machine binding |

<!-- reach-argument-tags -->
| Tag | Reach argument | Fields after the tag |
| --- | --- | --- |
| 0 | Type | identity string |
| 1 | Const | identity string |
| 2 | Proposition | identity string |
| 3 | Machine | identity string + 32-byte contract commitment |

## Proof bundle

A proof bundle is `PSIPRF\0\0` + `u16` format marker 33 + counted obligation
evidence rows + counted recursive component certificates + counted control
cycle certificates + counted evidence producers. Sealed for transport it is
`PSIPSC\0\0` + `u16` section marker 1 + `u16` vocabulary marker + a 32-byte
program fingerprint + the bundle bytes.

An obligation evidence row is an obligation id + a `u8` route tag: 1
KernelDerived + `u8` primitive judgment; 2 CertificateDerived + evidence
identity + `u16` proof-system marker + proof node; 3 Admitted + admission
site id + admission kind + authority identity + evidence identity + profile
decision id. A component certificate is a certificate identity + ranking
relation id + well-foundedness evidence route + counted edges (obligation id
+ evidence route). An evidence producer is a producer id + term id +
conformance string + trait string + counted rows (declaring-trait string +
counted argument strings + requirement string + machine string + state
string + `u8` source: 1 Inline, 2 Reference, 3 TraitDefault).

A proof node is its conclusion proposition, a `u8` rule tag, the node's
children — each recursively a proof node, written before the parent suffix —
then the rule suffix fields. Rule 4 writes a counted child list; rules 16 and
23 write one child (the disjunction or premise) then a counted continuation
list; all other rules carry a fixed child count given below. The proof tree
rejects nesting deeper than 256.

<!-- proof-rule-tags -->
| Tag | Proof rule | Children | Suffix fields |
| --- | --- | --- | --- |
| 1 | Primitive | 0 | `u8` primitive judgment (1 Truth, 2 ReflexiveEquality, 3 ClosedIntegerRelation, 4 IntegerCarrierBound) |
| 2 | SemanticAxiom | 0 | `u32` axiom index |
| 3 | Assumption | 0 | `u32` assumption index |
| 4 | ConjunctionIntroduction | counted | — |
| 5 | ConjunctionElimination | 1 | `u32` conjunct index |
| 6 | ImplicationIntroduction | 1 | — |
| 7 | ImplicationElimination | 2 (implication, premise) | — |
| 8 | EqualityTransitivity | 2 (left-middle, middle-right) | — |
| 9 | DisjunctionIntroduction | 1 | `u32` disjunct index |
| 10 | IntegerLessOrEqualTransitivity | 2 | — |
| 11 | IntegerOrderSubstitution | 2 | `u32` substituted endpoint index |
| 12 | IntegerAffineBound | 1 | root scalar term + target scalar term + counted definition axiom indices + counted optional literal axioms (`u8`: 0 absent; 1 + `u32` index) |
| 13 | IntegerCastBound | 1 | root scalar term + target scalar term + counted definition axiom indices |
| 14 | IntegerCorrelatedForbiddenRoots | 0 | dividend branch + divisor branch + `u32` definition axiom count + `u32` lower-bound axiom index + `u32` upper-bound axiom index + conclusion proposition; each branch is a root scalar term + target scalar term + counted steps (`u32` definition axiom index + optional literal axiom) |
| 15 | IntegerExactAddDefinitionBound | 2 (left bound, right bound) | `u32` definition axiom index |
| 16 | DisjunctionElimination | 1 + counted | — |
| 17 | EqualitySymmetry | 1 | — |
| 18 | IntegerOrderWeakening | 1 | — |
| 19 | IntegerOrderDiscreteness | 1 | — |
| 20 | IntegerSubtractOrder | 2 (difference, positive) | — |
| 21 | IntegerStrictOrderTransitivity | 2 | — |
| 22 | PredicateDenotation | 1 | — |
| 23 | ValueEqualityTransport | 1 + counted | — |

## Obligation ledger

The replay ledger binds the exact semantic subject and reconstruction trust
graph and deliberately excludes proof routes: different valid certificates may
discharge the same reconstructed question. Its bytes are `PSIOBLG\0` + `u16`
format marker 3 + `u16` vocabulary marker + the 32-byte semantic-module
fingerprint + the 32-byte trust graph identity + counted obligation rows.
Decoding rejects trailing bytes, duplicate obligation identities, and
duplicate owners, and the rows must re-encode byte-for-byte; a decoded ledger
is then compared against local obligation reconstruction rather than trusted
on arrival.

An obligation row is its owner, obligation id, class, conclusion proposition,
counted requirement propositions, counted semantic-axiom propositions, and a
`bool` canonical-certificate flag.

<!-- ledger-owner-tags -->
| Tag | Obligation owner | Fields after the tag |
| --- | --- | --- |
| 1 | Operation | machine id + operation id |
| 2 | CallRequires | machine id + operation id + `u32` requirement position |
| 3 | NominalCleanupRequires | machine id + edge id + `u32` cleanup position + `u32` requirement position |
| 4 | ContractEnsures | machine id + contract id + `u32` clause position |
| 5 | ScalarBlockInvariant | machine id + header block id + edge id |

<!-- obligation-class-tags -->
| Tag | Obligation class | Fields after the tag |
| --- | --- | --- |
| 1 | Derivable | — |
| 2 | AdmissionAuthorized | admission site id + admission kind + authority evidence identity |

## Section identities

| Section | Identity and role |
| --- | --- |
| Semantic module | Domain-separated commitment to exact canonical bytes; excludes replaceable proof, installation, and debug evidence. |
| Proof bundle | Independently identified evidence, published only when requested in `<artifact>.proof`, binding the semantic artifact and exact proof profile/dependencies. The current bounded `PSIPRF\0\0` encoding is not the complete general sidecar schema. |
| Obligation ledger | Independently identified reconstructed-obligation binding to the semantic subject and trust graph; proof routes stay replaceable evidence. |
| Optimization execution | Selection/output semantic provenance is rejoined at decoding. Internal proof identities and portable preservation evidence remain distinct; absent PCC does not waive transformation checking. |
| Installation | Separate `PSIINST\0` bytes and identity, retaining realization evidence without granting admission. |
| Debug map | Replaceable presentation metadata bound to the exact semantic subject, never program meaning. |

The reconstructed manifest binds each present component under its own hash domain.
Absent differs from present-but-empty. Replacing valid nonsemantic evidence
preserves semantic identity while changing its own identity. Proof sidecars do
not create an embedded proof-section requirement or alter unchanged artifact
bytes. [PCC publication](../proofs/publication.md) owns opt-in and exact companion
binding. Migrate the current embedded proof-envelope route; it is not a second
permanent distribution format. Physical sidecar tables remain execution work,
not permission to reinterpret existing bytes under an old format marker.

Proof evidence is strictly ordered by obligation identity and retains exact
rules, proof trees, and admissions. Preserve cited rule direction even though
proof bytes are replaceable. Disjunction introduction retains one checked child
and its selected canonical arm; absent/out-of-range arms and a child concluding
another arm reject. A proof-calculus constructor does not itself authorize a
semantic-ledger rule or an unproved reduction procedure.

## Retained placement custody

A source-derived placed-view input binds Terminal machine/state/parameter
coordinates to hermetic source, policy, producing-plan-machine, and schema
identities; the policy/schema-derived view identity; exact access and binding
modes; a report-only compact coordinate; and the validated plan's
domain-separated layout/access/reach commitment.

Reject missing machines, owned access in this view role, non-hermetic identities,
zero required report coordinates or commitments, duplicates, and noncanonical
order. The row grants no runtime storage, accessor, provider, physical address,
or ABI authority.

## Installation and semantic-code attribution

Installation bytes bind semantic identity, target facts, exact profile/provider
decisions, complete emitted-image hash, and text-validation evidence. Retain
separate domain-framed strong digests for encoded compiler text, final compiler
text, canonical relocations, and their derivation. Compact fingerprints are
report compatibility, not substitutes for those commitments. Installation still
consumes separate admission and placement authority; decoding yields an audit
projection, not an executable grant.

Effectful roots additionally retain the canonical function map, each privileged
port effect's service/operation/byte range, and each boundary settlement's exact
admitted execution binding and associated preceding realization. A settlement
emits no duplicate hardware effect. Reject missing, reordered, byte-drifted, or
raw-number-only realization evidence. Production uses the same admitted
provider executions as lowering and checks the complete emitted settlement
closure.

Emitted operations and return edges retain semantic site, operation ordinal,
function-relative offset, and byte count. Metadata-only settlement rows have
zero-byte intervals. This is replay/analysis provenance, not native instruction
cost or runtime charging. Structural call placement must preserve the
[borrow identity contract](structural_access.md);
it cannot stage every borrowed parameter as an owned copy merely because the
installation record can describe one.
