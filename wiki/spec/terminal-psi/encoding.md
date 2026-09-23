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
qualification catalog follows the entry identity as five counted tables:

| Catalog table | Row fields |
| --- | --- |
| domain definitions | domain id + semantic domain id + canonical identity string + scalar carrier type + counted establishment routes |
| normalized sets | `u64` set identity + counted domain identities |
| qualification-change edges | machine id + edge id + `u32` argument ordinal + source value id + destination value id |
| float entry ranges | machine id + parameter value id + minimum IEEE float value + maximum IEEE float value + `bool` maximum-inclusive flag |
| integer entry ranges | machine id + parameter value id + integer type + minimum integer value + maximum integer value |

Each scalar domain establishment route is a `u8` tag:

<!-- scalar-domain-establishment-route-tags -->
| Tag | Scalar domain establishment route | Fields after the tag |
| --- | --- | --- |
| 0 | CheckedRequirement | requirement identity string |
| 1 | BoundaryRequirement | requirement identity string |
| 2 | ExactMachine | machine identity string |

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
| scalar field carrier path | counted segments, each `u8` 1 + field id or 2 + `u64` fixed index; record carriers only — other segment tags reject |
| structural access | `u8`: 1 owned, 2 shared borrow, 3 mutable borrow, 4 write-only borrow |
| structural multiplicity | `u8`: 1 unrestricted, 2 affine, 3 linear |
| structural argument | place id + structural access + structural path |
| value declaration | value id + scalar type + `u64` qualification-set identity |
| erased proof formal | `u32` source position + type identity string |
| projected qualification | structural path + domain id |
| structural operation result | place id + structural type id + multiplicity + counted domain ids + counted projected qualifications + counted qualification establishments (domain id + `u64` route index) + counted claims (claim id + structural path) |
| structural parameter | place id + `u32` position + `u8` self flag + structural type id + multiplicity + access + counted domain ids + counted projected qualifications |
| scalar term list | counted scalar terms |
| proof term list | counted proof terms |
| obligation id list | counted obligation ids |
| claim transfer | claim id + `u32` argument index |
| returned claim transfer | callee claim id + caller claim id |
| completion receipt | claim id + `u32` argument index |
| crash routes | counted buckets, each `u8` cause (1 Trap, 2 Abort) + counted guards; each guard `u8`: 0 truth, 1 + proposition |
| successor edge | edge id + target block id + counted value ids + scalar term list + proof term list + counted structural arguments + counted trivial discards |
| trivial discard | place id; appears only inside a counted roster |
| residual discard | place id + structural path + structural type id |
| affine cleanup action | `u8`: 1 + place id; 2 + residual discard; 3 + place id + structural type id + machine id + optional identity + obligation id list |
| evidence interface | trait string + counted argument strings + counted requirements (declaring-trait string + counted argument strings + requirement string) |

The closed `u8` spaces behind the scalar field forms:

<!-- scalar-type-tags -->
| Tag | Scalar type | Fields after the tag |
| --- | --- | --- |
| 1 | Boolean | — |
| 2 | Integer | integer type |
| 3 | IeeeFloat | IEEE float format |

<!-- integer-type-tags -->
| Tag | Integer type | Fields after the tag |
| --- | --- | --- |
| 1 | Signed | `u16` bit width |
| 2 | Unsigned | `u16` bit width |
| 3 | Address | `u16` bit width |

<!-- integer-value-tags -->
| Tag | Integer value | Fields after the tag |
| --- | --- | --- |
| 1 | Signed | `i128` |
| 2 | Unsigned | `u128` |

<!-- ieee-float-value-tags -->
| Tag | IEEE float value | Fields after the tag |
| --- | --- | --- |
| 1 | Binary32 | `u32` bits |
| 2 | Binary64 | `u64` bits |

<!-- ieee-float-relation-tags -->
| Tag | IEEE float relation |
| --- | --- |
| 0 | Equal |
| 1 | NotEqual |
| 2 | Less |
| 3 | LessOrEqual |
| 4 | Greater |
| 5 | GreaterOrEqual |

<!-- ieee-comparison-kind-tags -->
| Tag | IEEE float comparison kind |
| --- | --- |
| 1 | Equal |
| 2 | NotEqual |

<!-- boolean-tags -->
| Tag | Boolean |
| --- | --- |
| 0 | False |
| 1 | True |

<!-- optional-identity-tags -->
| Tag | Optional identity | Fields after the tag |
| --- | --- | --- |
| 1 | Present | `id` |

<!-- scalar-field-carrier-path-tags -->
| Tag | Scalar field carrier segment | Fields after the tag |
| --- | --- | --- |
| 1 | Field | field id |
| 2 | FixedIndex | `u64` index |

The optional-identity byte 0 records absence and carries no fields; the
shared optional-identity reader decodes it as the absent form rather than a
table row.

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

## Proof terms

A proof term is a `u8` form tag followed by its fields. Proof terms are
proof-only erased actuals — they carry semantic identities for the erased
proof-formal lane, not runtime values. They appear only inside a proof term
list in the semantic module, and their nesting shares the scalar-term depth
bound.

<!-- proof-term-tags -->
| Tag | Proof term | Fields after the tag |
| --- | --- | --- |
| 1 | Construction | type identity string + `bool` case-identity flag (+ case identity string when set) + counted fields (field identity string + proof term) |
| 2 | Formal | `u32` position |

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
| 17 | ScalarIeeeFloatComparison | IEEE float comparison kind + IEEE float format + scalar term + scalar term |

## Operation rows

A block row is a block id, counted parameter value declarations, counted erased
scalar formal declarations, counted erased proof formals, counted structural
parameters, counted operation rows, and one terminator row.

An operation row is an operation id, a `u8` static-reach flag (1 adds a `u32`
argument binding; 0 records none), a `u8` result tag (0 unit; 1 + value
declaration; 2 + structural operation result), the `u8` operation tag, that
tag's fields, and a trailing `u8` possibly-suspending crossing flag (1 adds a
crossing id binding the call-side suspension demand; 0 records none).

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
| 33 | Call | callee machine id + counted value ids + scalar term list + proof term list + obligation id list + crash routes |
| 34 | CallUnit | callee machine id + counted value ids + scalar term list + proof term list + counted structural arguments + counted claim transfers + obligation id list + crash routes |
| 35 | BoundaryCall | boundary machine id + counted value ids + counted structural arguments + counted completion receipts |
| 36 | PortWrite | service id + `u16` port + `u8` value |
| 37 | EstablishTrivialAffineLocal | place id |
| 38 | BooleanStructuralField | source place id + scalar field carrier path + field id |
| 39 | CallStructuralScalar | callee machine id + counted value ids + scalar term list + proof term list + counted structural arguments + counted claim transfers + obligation id list + crash routes |
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
| 50 | CallStructuralWithScalarArguments | callee machine id + counted value ids + scalar term list + proof term list + counted structural arguments + counted claim transfers + counted returned claim transfers + obligation id list + crash routes |
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
| 79 | EstablishElementView | destination place id + source place id + element type id |
| 80 | ElementViewLength | source place id |
| 81 | ElementViewRead | source place id + index value id + length value id + obligation id |
| 82 | ElementViewSubslice | source place id + start value id + end value id + length value id + obligation id |

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

The result byte in the operation row header and the operand bytes nested in
tag-68 and tag-41 rows are their own closed `u8` spaces:

<!-- operation-result-tags -->
| Tag | Operation result | Fields after the tag |
| --- | --- | --- |
| 0 | Unit | — |
| 1 | Scalar | value declaration |
| 2 | Structural | structural operation result |

<!-- record-field-value-tags -->
| Tag | Record field operand | Fields after the tag |
| --- | --- | --- |
| 1 | Scalar | value id + optional obligation id |
| 2 | Structural | structural argument |

<!-- outcome-result-substitution-tags -->
| Tag | Outcome result substitution | Fields after the tag |
| --- | --- | --- |
| 0 | Absent | — |
| 1 | Present | `u32` argument position + callee result place id + caller result place id |

## Terminator rows

A terminator row is a `u8` tag followed by its fields.

<!-- terminator-tags -->
| Tag | Terminator | Fields after the tag |
| --- | --- | --- |
| 1 | Jump | edge id + target block id + counted value ids + scalar term list + proof term list + counted structural arguments + counted trivial discards |
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

The affine cleanup actions a terminator carries are a closed `u8` space:

<!-- affine-cleanup-action-tags -->
| Tag | Affine cleanup action | Fields after the tag |
| --- | --- | --- |
| 1 | DiscardRoot | place id |
| 2 | DiscardResidual | residual discard |
| 3 | InvokeNominal | place id + structural type id + machine id + optional identity + obligation id list |

## Machine rows

A boundary declaration stores its counted runtime parameter-order tags after
the optional attachment and before scalar parameter types: `u8` 0 selects the
next scalar, 1 the next structural parameter. Unknown tags, missing entries, and
lane-count mismatches reject. Reordering a valid roster changes the semantic
identity even when the parameters have identical physical shapes.

Module bytes are `PSITERM\0` + `u16` format marker 105 + `u16` vocabulary
marker 107 + the entry machine id, followed by the module's counted tables in
the declaration order below and ending with the machine roster.

<!-- module-table-order -->
| # | Module table | Row |
| --- | --- | --- |
| 1 | scalar qualification catalog | the five counted catalog tables above |
| 2 | structural types | counted structural type declarations |
| 3 | structural domains | counted structural domain declarations |
| 4 | services | counted service declarations |
| 5 | concrete root service reach | counted service ids |
| 6 | installation reach dependencies | counted installation reach rows |
| 7 | placed-view inputs | counted placed-view input rows |
| 8 | reborrow root handoffs | counted reborrow handoff rows |
| 9 | reborrow restored call uses | counted restored-use rows |
| 10 | boundary machines | counted boundary machine declarations |
| 11 | provider candidates | counted provider candidate rows |
| 12 | float-meaning projections | counted projection rows |
| 13 | float-meaning equalities | counted equality rows |
| 14 | proposition declarations | counted proposition declaration rows |
| 15 | proposition applications | counted proposition application rows |
| 16 | evidence terms | counted evidence term rows |
| 17 | evidence contract lanes | counted evidence lane rows |
| 18 | proof-output invocations | counted proof-output call rows |
| 19 | proof recursive components | counted recursive component rows |
| 20 | closed conformance applications | counted conformance application rows |
| 21 | dynamic descriptor parameters | counted descriptor parameter rows |
| 22 | dynamic descriptor arguments | counted descriptor argument rows |
| 23 | dynamic conformance selections | counted conformance selection rows |
| 24 | rebound dynamic descriptors | counted rebound descriptor rows |
| 25 | stored dynamic descriptors | counted stored descriptor rows |
| 26 | direct dynamic dispatches | counted direct dispatch rows |
| 27 | indirect dynamic dispatches | counted indirect dispatch rows |
| 28 | stored dynamic dispatches | counted stored dispatch rows |
| 29 | parameter dynamic dispatches | counted parameter dispatch rows |
| 30 | suspension rows | a bare `u32` call-plan count, then counted suspension call sites, then counted suspension call plans |
| 31 | quotient correspondences | counted quotient correspondence rows |
| 32 | scalar block invariants | the counted invariant rows above |
| 33 | operation crash contracts | counted operation crash contract rows |
| 34 | machines | counted machine rows |

The machine roster is strictly
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
| machine contract | contract id + crash routes + counted erased scalar formal declarations + counted erased proof formals + counted requires propositions + counted ensures clauses + counted outcome-specific ensures |
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
its version byte + root place id + counted segments. A structural place kind
byte inside a content row shares the place-kind grammar with tag 8 retired —
BlockParameter places are machine-local and reject at content positions:

<!-- content-structural-place-kind-tags -->
| Tag | Content structural place kind | Fields after the tag |
| --- | --- | --- |
| 1 | Parameter | `u32` position + `bool` self flag |
| 2 | Result | — |
| 3 | TrivialAffineLocal | `u32` declaration ordinal + structural type id; no construction element |
| 4 | ByteSequenceLiteral | `u32` declaration ordinal + structural type id |
| 5 | ProviderAttachment | attachment structural type id + field id + boundary machine id |
| 6 | OperationResult | producer operation id + structural type id |
| 7 | TrivialAffineLocal | `u32` declaration ordinal + structural type id + root structural type id + `u64` construction index |
| 8 | — | retired at content positions: BlockParameter places are machine-local |

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

## Module declaration tables

The counted module tables that precede the machine roster carry the module's
declarations, custody evidence, and certificate indexes. Their rows are
byte-encoded exactly as below; a receiver decodes them with no access to
producer state and rederives every ordering rule.

### Structural declarations

| Row | Fields |
| --- | --- |
| structural type declaration | structural type id + identity string + structural type shape |
| structural field | structural field id + identity string + binding relevance + structural field type |
| structural case | structural case id + identity string + counted payload structural fields |
| structural domain declaration | domain id + semantic domain id + identity string + carrier structural type id + optional content projection + counted establishment routes |
| optional content projection | `u8` 0 absent; `u8` 1 + projection domain id + `u64` projection report fingerprint + content algebra + content projection expression |
| structural establishment route | `u8` tag per the establishment-route table |
| service declaration | service id + identity string + counted parent service ids |
| concrete root service reach | counted service ids |
| installation reach dependency | requirement identity string + counted service-id upper bound |

Structural type declarations are strictly ordered by structural type id; a
declaration's record and mixed fields are strictly ordered by structural field
id, its sum and mixed cases by structural case id, and each case's payload
fields by structural field id. Domains are strictly ordered by domain id.
Services are strictly ordered by service id with strictly ordered parents.
The concrete root service reach is strictly ordered, and installation reach
dependencies are strictly ordered by requirement identity with each upper
bound strictly ordered.

<!-- structural-type-shape-tags -->

| tag | Structural type shape |
| --- | --- |
| 1 | Record | counted structural fields |
| 2 | FixedArray | element structural type id + `u64` length |
| 3 | Sum | counted structural cases |
| 4 | ByteSequence | byte-sequence carrier |
| 5 | Mixed | counted structural fields + counted structural cases |
| 6 | PrimitiveScalar | scalar type |
| 7 | Reference | referent structural type id + structural access |
| 8 | ElementView | element structural type id |

<!-- byte-sequence-carrier-tags -->

| tag | Byte-sequence carrier |
| --- | --- |
| 1 | BorrowedView |
| 2 | BoundedOwned | `u64` capacity |

<!-- binding-relevance-tags -->

| tag | Binding relevance |
| --- | --- |
| 1 | Relevant |
| 2 | Erased |

<!-- structural-field-type-tags -->

| tag | Structural field type |
| --- | --- |
| 1 | Scalar | scalar type |
| 2 | Structural | structural type id |
| 3 | Erased | type identity string |
| 4 | IeeeFloat | IEEE float format |
| 5 | ByteSequence | byte-sequence carrier |
| 6 | BoundedInteger | integer type + integer value minimum + integer value maximum |

<!-- structural-access-tags -->

| tag | Structural access |
| --- | --- |
| 1 | Owned |
| 2 | SharedBorrow |
| 3 | MutableBorrow |
| 4 | WriteOnlyBorrow |

<!-- structural-multiplicity-tags -->

| tag | Structural multiplicity |
| --- | --- |
| 1 | Unrestricted |
| 2 | Affine |
| 3 | Linear |

<!-- structural-path-segment-tags -->

| tag | Structural path segment | Fields after the tag |
| --- | --- | --- |
| 1 | Field | field identity string |
| 2 | FixedIndex | `u64` index |
| 3 | Referent | none |
| 4 | FixedByteRange | `u64` start + `u64` end |

`FixedByteRange` retains the exact half-open byte window `[start, end)` of
initialized fixed-array backing for one borrowed call argument. Both endpoints
participate in canonical identity, including empty windows. This terminal
segment follows the backing's field-only path; it does not identify an owned
subtree or an escaping reference. The call's type, access, bounds, and overlap
checks remain required by [byte views](byte_views.md#calls-and-control-transfers).

<!-- canonical-path-segment-tags -->

| tag | Canonical structural path segment |
| --- | --- |
| 1 | Field | structural field id |
| 2 | FixedIndex | `u64` index |
| 3 | Case | structural case id |

<!-- structural-establishment-route-tags -->

| tag | Structural establishment route | Fields after the tag |
| --- | --- | --- |
| 1 | Requirement | requirement identity string |
| 2 | ExactMachine | machine identity string |
| 3 | BoundaryRequirement | requirement identity string |

A canonical structural field path is a root place id + counted canonical path
segments. An IEEE float field path is a root place id + counted canonical
path segments.

<!-- content-projection-expression-tags -->

| tag | Content projection expression |
| --- | --- |
| 1 | IntervalSet | counted (start capacity scalar + end capacity scalar) bounds |
| 2 | CountedQuantity | capacity scalar |

A capacity scalar inside a domain declaration is a `u8` tag:

<!-- content-projection-scalar-tags -->

| tag | Content projection scalar | Fields after the tag |
| --- | --- | --- |
| 1 | SubjectField | counted field path strings |
| 2 | RuntimeScalarEmbedding | counted field path strings |
| 3 | Natural | natural string |
| 4 | Successor | capacity scalar |
| 5 | Add | two capacity scalars |
| 6 | Subtract | two capacity scalars |
| 7 | Multiply | two capacity scalars |

### Placed-view inputs and reborrow custody

These rosters are positional: entries decode in producer-declared order and
are not re-sorted by the canonical-order checker.

| Row | Fields |
| --- | --- |
| placed-view input | machine id + `u32` position + source machine identity string + source state identity string + source parameter identity string + structural access + `bool` const binding + `bool` mutable binding + view identity string + policy identity string + policy-plan machine identity string + schema identity string + `u64` placement report fingerprint + 32-byte placement commitment |
| reborrow root handoff | machine id + source machine identity string + source state identity string + direct root owner identity string + counted owner path segments + direct root place + direct root access + activation boundary + weakening boundary + direct-root lifetime identity string + counted lineage steps |
| reborrow lineage step | child owner identity string + counted owner path segments + child place + counted projection remainder segments + child access + child activation boundary + formation boundary + child weakening boundary |
| reborrow place | root identity string + counted place segments |
| reborrow restored call use | machine id + operation id + restoration class + call boundary + call target machine id + source machine identity string + source state identity string + direct root owner identity string + counted owner path segments + direct root place + activation boundary + weakening boundary + direct-root lifetime identity string + child owner identity string + counted owner path segments + child place + counted projection remainder segments + child access + child activation boundary + formation boundary + child weakening boundary + counted shared cohort |
| shared cohort member | child owner identity string + counted owner path segments + child place + child access + child activation boundary + child weakening boundary |

<!-- borrow-boundary-tags -->

| tag | Borrow boundary |
| --- | --- |
| 1 | Statement | `u64` statement index |
| 2 | Call | `u64` statement index + `u64` call ordinal + target identity string |

<!-- borrow-owner-segment-tags -->

| tag | Borrow owner path segment |
| --- | --- |
| 1 | Field | field name string |
| 2 | Case | case name string |
| 3 | FixedIndex | `u64` index |
| 4 | DynamicIndex |

<!-- borrow-place-segment-tags -->

| tag | Borrow place segment |
| --- | --- |
| 1 | Field | field name string |
| 2 | Case | case name string |
| 3 | FixedIndex | `u64` index |
| 4 | FixedRange | `u64` start + `u64` end |

<!-- restoration-class-tags -->

| tag | Reborrow restoration class |
| --- | --- |
| 1 | ExclusiveReactivation |
| 2 | SharedFreezeRestoration |

### Boundary machines and provider candidates

| Row | Fields |
| --- | --- |
| boundary machine | boundary machine id + identity string + optional attachment structural type id + counted parameter order + counted scalar parameter types + crash routes + counted structural parameters + result + counted structural requirements + counted program-local root introductions + counted content guarantees + fixed service reach + published service ceiling |
| optional structural type id | `u8` 0 absent; `u8` 1 + structural type id |
| structural requirement | `u32` argument index + domain id |
| program-local root introduction | `u32` argument index + `u32` source parameter position + qualification domain id + carrier structural type id + projection domain id + `u64` projection report fingerprint + content algebra + content projection expression + `u64` compatibility report identity |
| structural parameter | the machine-row structural parameter encoding |
| service reach | counted service ids |
| provider candidate | boundary machine id + requirement identity string + provider identity string + candidate identity string + candidate machine id + counted signature parameters + counted positional refinements + counted required domains + counted realized service-ceiling ids |
| signature parameter | `u32` position + `bool` self + structural type id + structural multiplicity + structural access + counted qualification domain ids + counted projected qualifications |
| projected qualification | structural path + domain id |
| positional refinement | `u32` boundary parameter index + `u32` candidate parameter index |
| required domain | `u32` argument index + domain id |

<!-- boundary-parameter-kind-tags -->

| tag | Boundary parameter kind |
| --- | --- |
| 0 | Scalar |
| 1 | Structural |

<!-- boundary-result-tags -->

| tag | Boundary machine result |
| --- | --- |
| 0 | Unit |
| 1 | Scalar | scalar type |
| 2 | Structural | structural type id + structural multiplicity + counted qualification domain ids |

<!-- boundary-content-guarantee-tags -->

| tag | Boundary content guarantee |
| --- | --- |
| 1 | Conservation | content conservation guarantee |
| 2 | RetainedBorrow | retained-borrow custody |

A content conservation guarantee is a `u64` report fingerprint + counted
structural places (each a place id + content structural place kind) + a
content conservation (content algebra + content term + content term).

| Row | Fields |
| --- | --- |
| retained-borrow custody | callable identity string + source retained-borrow place + result retained-borrow place + structural access + `u32` callable lifetime parameter count + `u32` callable lifetime parameter ordinal + result nominal identity string + result structural multiplicity + `u32` result lifetime argument count + `u32` result lifetime argument ordinal + `bool` result lifetime slot erased + retained semantic domain id + source retained-borrow projection + result retained-borrow projection |
| retained-borrow place | content place version + retained-borrow root + counted retained-borrow segments |
| retained-borrow projection | semantic domain id + carrier identity string + domain id + `u64` report fingerprint + content algebra + content projection expression |

<!-- retained-borrow-root-tags -->

| tag | Retained-borrow root |
| --- | --- |
| 1 | Parameter | `u32` position + identity string + `bool` self |
| 2 | Result |

<!-- retained-borrow-segment-tags -->

| tag | Retained-borrow place segment |
| --- | --- |
| 1 | Case | case name string |
| 2 | Field | field name string |
| 3 | FixedIndex | `u64` index |

Retained-borrow place segments order Case before Field — deliberately the
reverse of the content-place segment table; the two tag spaces are
independent and a receiver must not share the decode table.

Boundary machines are strictly ordered by boundary machine id with canonical
crash routes, dense parameter positions, strictly ordered requirements and
strictly ordered service ceilings. Provider candidates are strictly ordered
by boundary machine id, then provider identity, then candidate identity, then
candidate machine id.

### Float-meaning rows

| Row | Fields |
| --- | --- |
| float-meaning projection | `u32` proof value id + proof-only value type + float-meaning source + projection operation + projection contract |
| projection contract | `u16` float format + `u8` operation + `u8` declaration + `u16` catalog version + 32-byte commitment |
| float semantic contract identity | `u8` catalog row + `u16` catalog version + 32-byte commitment |
| float-meaning equality | `u32` proposition id + `u32` left proof value + `u32` right proof value |
| IEEE float field | root place id + counted canonical path segments |

<!-- float-value-type-tags -->

| tag | Proof-only value type |
| --- | --- |
| 1 | FloatMeaning |

<!-- float-meaning-source-tags -->

| tag | Float-meaning source |
| --- | --- |
| 1 | TransitionalInput | `u32` input id + IEEE float format |
| 2 | ExactBinary32Literal | `u32` bit pattern |
| 3 | ExactBinary64Literal | `u64` bit pattern |
| 4 | DirectMachineParameter | owner machine id + parameter value id + IEEE float format |
| 5 | DirectMachineResult | owner machine id + result id + IEEE float format |
| 6 | DirectOperationResult | owner machine id + producer operation id + result id + IEEE float format |
| 7 | DirectBlockParameter | owner machine id + block id + block parameter id + IEEE float format |
| 8 | DirectCallResult | owner machine id + producer operation id + call result id + IEEE float format |
| 9 | DirectStructuralLeaf | owner machine id + IEEE float field + IEEE float format |
| 10 | SemanticApplication | float semantic contract identity + IEEE float format + counted operands |

<!-- ieee-format-tags -->

| tag | IEEE float format |
| --- | --- |
| 1 | Binary32 |
| 2 | Binary64 |

<!-- float-operand-tags -->

| tag | Float semantic operand |
| --- | --- |
| 1 | Format | IEEE float format |
| 2 | Meaning | `u32` proof value id |

<!-- float-projection-operation-tags -->

| tag | Float-meaning projection operation |
| --- | --- |
| 1 | Meaning32 |
| 2 | Meaning64 |

Projection rows carry dense proof value ids equal to their table position;
TransitionalInput source ids are dense in first-use order across the whole
roster. Equality rows carry dense proposition ids equal to their position and
ordered operands. A projection source's IEEE float format is redundant with
the referenced semantic contract row and is checked against it.

### Proposition and evidence declarations

| Row | Fields |
| --- | --- |
| proposition declaration | proposition id + proposition name string + counted binders + counted parameter type strings + proposition evidence |
| proposition binder | binder name string + binder kind |
| proposition application | proposition id + declaration id + counted binder arguments + counted argument strings + optional evidence interface |
| binder argument | binder argument kind + `u8` selector (0 + identity string; 1 + evidence projection) |
| evidence projection | evidence term id + declaring trait identity string + counted declaring trait argument strings + requirement identity string |
| evidence interface | trait identity string + counted argument strings + counted requirements |
| evidence interface requirement | declaring trait identity string + counted declaring trait argument strings + requirement identity string |
| evidence term | evidence term id + proposition id + evidence interface |
| evidence contract lane | machine id + lane kind + `u32` position + evidence term id + optional output field string |

<!-- proposition-binder-kind-tags -->

| tag | Proposition binder kind |
| --- | --- |
| 1 | Type |
| 2 | Const | type identity string |
| 3 | Machine |

<!-- proposition-evidence-tags -->

| tag | Proposition evidence |
| --- | --- |
| 1 | FactOnly |
| 2 | Witness | evidence type identity string |

<!-- binder-argument-kind-tags -->

| tag | Binder argument kind |
| --- | --- |
| 1 | Type |
| 2 | Const |
| 3 | Machine |

<!-- proposition-binder-argument-tags -->

| tag | Binder argument selector | Fields after the tag |
| --- | --- | --- |
| 0 | Identity | identity string |
| 1 | EvidenceProjection | evidence projection |

<!-- proposition-evidence-interface-tags -->

| tag | Evidence interface selector | Fields after the tag |
| --- | --- | --- |
| 0 | Absent | — |
| 1 | Present | evidence interface |

<!-- evidence-lane-kind-tags -->

| tag | Evidence contract lane kind |
| --- | --- |
| 1 | Requires |
| 2 | Ensures |

Proposition declarations and applications are strictly ordered by semantic
identity and by proposition id; application and evidence-interface selectors
encode each argument's provenance (0 source identity string, 1 evidence
projection). Evidence terms are strictly ordered by proposition id,
interface, and term id and separately by term id; lanes are strictly ordered
by machine id, lane kind, and position.

### Proof-output invocations and recursive components

| Row | Fields |
| --- | --- |
| proof-output invocation | caller machine id + `u32` ordinal + target machine identity string + optional static requirement dispatch + optional runtime result + optional runtime call + counted evidence arguments + counted outputs |
| static requirement dispatch | `u64` conformance application report fingerprint + 32-byte commitment + public requirement identity + declaring trait identity + requirement identity + realization identity + realization callable identity + realization machine id |
| runtime result | `u8` presence; when present `u8` scalar flag (1 + scalar type; 0 Unit) |
| runtime call | `u8` presence; when present operation id + callee machine id |
| evidence argument | `u32` input position + callee proposition id + source evidence term id + instantiated proposition id |
| output | `u32` output position + output field string + callee proposition id + optional callee output evidence term id + instantiated proposition id + optional `u32` forwarded input position + optional evidence term id |
| proof recursive component | ranking relation + rank type identity string + counted member types + counted members + counted edges |
| member type | type identity string + counted fields (each field identity string + type identity string) |
| recursive component member | contract id + machine identity string + rank parameter identity string |
| recursive component edge | caller contract id + callee contract id + call site + counted strict member path strings |

Optional fields above are a `u8` 0/1 presence flag followed by the payload
when present. Proof-output invocations are strictly ordered by caller machine
id and ordinal, and each call's outputs are positional by output position.

<!-- ranking-relation-tags -->

| tag | Proof ranking relation |
| --- | --- |
| 1 | StructuralSubterm |

<!-- recursive-call-site-tags -->

| tag | Recursive proof call site |
| --- | --- |
| 1 | Statement | state identity string + `u64` statement index |
| 2 | Expression | state identity string + `u64` statement index + `u64` expression ordinal |
| 3 | Transition | state identity string + `u64` statement index + transition lane |

<!-- recursive-transition-lane-tags -->

| tag | Recursive transition lane |
| --- | --- |
| 1 | Target |
| 2 | Continuation |

### Closed conformance applications

| Row | Fields |
| --- | --- |
| closed conformance application | owner machine id + declaration identity string + counted telescope bindings + optional subject identity + trait identity string + counted trait lifetime argument strings + counted trait argument strings + counted realization callables + counted conformance rows + `u64` report fingerprint + 32-byte commitment |
| telescope binding | parameter string + parameter kind + argument string |
| realization callable | source callable identity string + machine id + callable result |
| conformance row | declaring trait identity string + public requirement identity string + counted family tuple strings + requirement identity string + realization identity string + optional realization callable identity string |

<!-- conformance-parameter-kind-tags -->

| tag | Conformance parameter kind |
| --- | --- |
| 1 | Lifetime |
| 2 | Type |
| 3 | Const |
| 4 | Machine |

<!-- callable-result-tags -->

| tag | Callable result |
| --- | --- |
| 1 | Unit |
| 2 | I32 |
| 3 | Bool |

Closed conformance applications are strictly ordered by owner machine id,
declaration identity, and report fingerprint.

### Dynamic dispatch tables

| Row | Fields |
| --- | --- |
| dynamic descriptor parameter | owner machine id + `u32` ordinal + `u32` source parameter position + trait identity string + structural access + counted requirements |
| descriptor requirement | `u32` requirement slot + declaring trait identity string + public requirement identity string + counted family tuple strings + callable result |
| dynamic descriptor argument | owner machine id + operation id + `u32` parameter ordinal + descriptor source |
| dynamic conformance selection | owner machine id + `u32` ordinal + counted structural arguments (exactly one) + `u64` conformance application report fingerprint + 32-byte commitment |
| structural argument | place id + structural access + counted structural path segments |
| rebound dynamic descriptor | owner machine id + `u32` ordinal + `u32` initial selection ordinal + `u32` rebound selection ordinal |
| stored dynamic descriptor | owner machine id + `u32` ordinal + establishment operation id + `u32` selection ordinal + aggregate type identity string + field identity string |
| direct dynamic dispatch | owner machine id + operation id + `u32` selection ordinal + realization targets |
| indirect dynamic dispatch | owner machine id + operation id + `u32` descriptor ordinal + realization targets |
| stored dynamic dispatch | owner machine id + operation id + `u32` descriptor ordinal + realization targets |
| parameter dynamic dispatch | owner machine id + operation id + `u32` parameter ordinal + `u32` requirement slot |
| realization targets | declaring trait identity + public requirement identity + counted family tuple strings + requirement identity + realization identity + realization callable identity + realization machine id |

<!-- descriptor-source-tags -->

| tag | Dynamic descriptor source |
| --- | --- |
| 1 | ReboundDescriptor | `u32` rebound descriptor ordinal |
| 2 | Parameter | `u32` descriptor parameter ordinal |
| 3 | Selection | `u32` conformance selection ordinal |

Descriptor parameters are strictly ordered by owner machine id and ordinal;
arguments by owner machine id, operation id, and parameter ordinal;
selections, rebound descriptors, and stored descriptors by owner machine id
and ordinal; and every dispatch table by owner machine id and operation id.

### Suspension rows

The suspension section is a bare `u32` call-plan count, then counted call
sites, then counted call plans.

| Row | Fields |
| --- | --- |
| suspension call site | operation id + crossing id + suspension call target + 32-byte frontier commitment |
| suspension call plan | operation id + crossing id + suspension call target + carry policy + `u32` live value count + counted live values |
| suspension live value | suspension place + suspension value type + suspension storage + `u32` claim count + counted claim ids + carry policy |
| carry policy | carry suspension + carry cpu + carry host thread + carry address (one tag byte each) |

<!-- suspension-target-tags -->

| tag | Suspension call target |
| --- | --- |
| 1 | Machine | machine id |
| 2 | Boundary | boundary machine id |
| 3 | DynamicDescriptor | `u32` descriptor ordinal |
| 4 | DynamicParameter | `u32` parameter ordinal + `u32` requirement slot |

<!-- carry-suspension-tags -->

| tag | Carry suspension |
| --- | --- |
| 1 | Forbidden |
| 2 | Allowed |

<!-- carry-cpu-tags -->

| tag | Carry cpu |
| --- | --- |
| 1 | Origin |
| 2 | Any |

<!-- carry-host-thread-tags -->

| tag | Carry host thread |
| --- | --- |
| 1 | Origin |
| 2 | Any |

<!-- carry-address-tags -->

| tag | Carry address |
| --- | --- |
| 1 | Stable |
| 2 | Movable |

<!-- suspension-place-tags -->

| tag | Suspension place |
| --- | --- |
| 1 | Scalar | value id |
| 2 | Structural | place id + counted structural path segments |

<!-- suspension-value-type-tags -->

| tag | Suspension value type |
| --- | --- |
| 1 | Scalar | scalar type |
| 2 | Structural | structural type id |

<!-- suspension-storage-tags -->

| tag | Suspension storage |
| --- | --- |
| 1 | Persistent |
| 2 | Parameter |
| 3 | Local |
| 4 | CallArgument |

Suspension call sites and plans are strictly ordered by operation id and
crossing id; each plan's live values are strictly ordered by place, storage,
value type, and carry policy, and each live value's claims are strictly
ordered by claim id.

### Quotient correspondences

| Row | Fields |
| --- | --- |
| quotient correspondence | operation kind + public callable + representative application + counted positional input relations + positional result relation + counted runtime positions + counted theorem evidence + representative eligibility + result flow positions |
| public callable | declaration identity string + overload identity string |
| representative application | public callable + counted static binding strings |
| positional relation | quotient declaration identity + quotient type identity + carrier type identity + relation callable identity strings (two) |
| runtime position | `u32` public position + `u32` representative position |
| theorem evidence | theorem role + selected application + theorem correspondence + quotient eligibility (purity + termination + crash) |
| representative eligibility | quotient purity + quotient termination |
| result flow positions | `u32` state position + `u32` statement position |
| congruence theorem | counted parameters + counted relation premises + counted legality premises + conclusion |
| theorem parameter | `u32` theorem parameter position + parameter role |
| relation premise | `u32` expected premise position + coordinate + relation string + `u32` left parameter + `u32` right parameter |
| transport fact | application side + source coordinate + actual coordinate |
| transport coordinate | contract owner + `u32` contract position + `u32` fact position |
| congruence conclusion | coordinate + relation string + left representative application + right representative application |
| theorem representative application | counted `u32` argument positions |

<!-- quotient-operation-kind-tags -->

| tag | Quotient operation kind |
| --- | --- |
| 1 | Lift |
| 2 | Define |
| 3 | LiftWithForwardPreconditionTransport |

<!-- quotient-positional-relation-tags -->

| tag | Quotient positional relation |
| --- | --- |
| 1 | Quotient | positional relation |
| 2 | ExactEquality | public type identity string + representative type identity string |

<!-- quotient-theorem-role-tags -->

| tag | Quotient theorem role |
| --- | --- |
| 1 | Congruence |
| 2 | ForwardPreconditionTransport |

<!-- quotient-theorem-correspondence-tags -->

| tag | Quotient theorem correspondence |
| --- | --- |
| 1 | Congruence | congruence theorem |
| 2 | ForwardPreconditionTransport | counted public premises + counted representative conclusions (each a counted transport facts list) |

<!-- quotient-purity-tags -->

| tag | Quotient purity |
| --- | --- |
| 1 | PureClosure |

<!-- quotient-termination-tags -->

| tag | Quotient termination |
| --- | --- |
| 1 | Unconditional |

<!-- quotient-crash-tags -->

| tag | Quotient crash |
| --- | --- |
| 1 | CrashFree |

<!-- quotient-parameter-role-tags -->

| tag | Quotient theorem parameter role |
| --- | --- |
| 1 | QuotientLeft | `u32` input position |
| 2 | QuotientRight | `u32` input position |
| 3 | Shared | `u32` input position |

<!-- quotient-application-side-tags -->

| tag | Quotient application side |
| --- | --- |
| 1 | Left |
| 2 | Right |

<!-- quotient-contract-owner-tags -->

| tag | Quotient contract owner |
| --- | --- |
| 1 | Machine |
| 2 | State |

A quotient correspondence's retained identity is derived from the
certificate: it is a pure function of the encoded row and is not itself
encoded. Rows are strictly ordered by that derived canonical identity.

### Operation crash contracts

| Row | Fields |
| --- | --- |
| operation crash contract | machine id + operation id + published crash routes + crash continuations |
| crash routes / continuations | counted crash route buckets |
| crash route bucket | crash cause + counted alternatives |

<!-- crash-cause-tags -->

| tag | Crash cause |
| --- | --- |
| 1 | Trap |
| 2 | Abort |

<!-- crash-route-guard-tags -->

| tag | Crash route guard |
| --- | --- |
| 0 | Truth |
| 1 | Predicate | proposition |

Crash route buckets are strictly ordered by crash cause; each bucket's
alternatives are nonempty and strictly increasing, with Truth permitted only
as the sole alternative. Predicate alternatives must be canonical
propositions. An operation's published crash routes must be nonempty.
Operation crash contracts are strictly ordered by machine id and operation
id.

## Proof bundle

A proof bundle is `PSIPRF\0\0` + `u16` format marker 33 + counted obligation
evidence rows + counted recursive component certificates + counted control
cycle certificates + counted evidence producers. Sealed for transport it is
`PSIPSC\0\0` + `u16` section marker 1 + `u16` vocabulary marker + a 32-byte
program fingerprint + the bundle bytes.

An obligation evidence row is an obligation id + an evidence route tag. A
component certificate is a certificate identity + ranking relation id +
well-foundedness evidence route + counted edges (obligation id + evidence
route). An evidence producer is a producer id + term id + conformance string
+ trait string + counted rows (declaring-trait string + counted argument
strings + requirement string + machine string + state string + row source).

<!-- evidence-route-tags -->
| Tag | Evidence route | Fields after the tag |
| --- | --- | --- |
| 1 | KernelDerived | primitive judgment |
| 2 | CertificateDerived | evidence identity + `u16` proof-system marker + proof node |
| 3 | Admitted | admission site id + admission kind + authority identity + evidence identity + profile decision id |

<!-- primitive-judgment-tags -->
| Tag | Primitive judgment |
| --- | --- |
| 1 | Truth |
| 2 | ReflexiveEquality |
| 3 | ClosedIntegerRelation |
| 4 | IntegerCarrierBound |

<!-- evidence-producer-row-source-tags -->
| Tag | Evidence producer row source |
| --- | --- |
| 1 | Inline |
| 2 | Reference |
| 3 | TraitDefault |

A proof node is its conclusion proposition, a `u8` rule tag, the node's
children — each recursively a proof node, written before the parent suffix —
then the rule suffix fields. Rule 4 writes a counted child list; rules 16 and
23 write one child (the disjunction or premise) then a counted continuation
list; all other rules carry a fixed child count given below. The proof tree
rejects nesting deeper than 256.

<!-- proof-rule-tags -->
| Tag | Proof rule | Children | Suffix fields |
| --- | --- | --- | --- |
| 1 | Primitive | 0 | primitive judgment |
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

## Canonical artifact

The transport envelope is `PSIART\0\0` + `u16` format marker 2 + the counted
section framing below; a receiver rejects an unknown magic, a stale marker,
an unknown debug-presence tag, and trailing bytes.

<!-- artifact-framing -->
| # | Framing field | Bytes |
| --- | --- | --- |
| 1 | semantic section length | `u64` byte length of the semantic module section |
| 2 | proof section length | `u64` byte length of the sealed proof section |
| 3 | optimization section length | `u64` byte length of the optimization execution section |
| 4 | debug presence | `u8`: 0 absent; 1 + `u64` debug section byte length |
| 5 | section bytes | semantic, sealed proof, optimization, then debug bytes when present, in that order |

Decoding is reconstruction: the semantic module validates on its own; the
sealed proof section decodes only against that decoded module's identity, so
the producer cannot choose the verifier's subject; the optimization and debug
sections rebind to it. The artifact manifest is rebuilt from the section
bytes, never transported. Every decoded section must re-encode to the exact
transported bytes — a producer's non-canonical serialization rejects.

## Debug map

A debug map is `PSIDBG\0\0` + `u16` format marker 1 + `u16` vocabulary marker
+ the 32-byte semantic-module fingerprint it presents + counted source files
+ counted sites. The fingerprint must equal the decoded module's identity:
presentation metadata binds the exact subject, never a different program.

A source file row is a `u32` file id + `u8` origin tag + `u64` byte length +
32-byte content digest + string path. A site row is a subject + `u32` file id
+ `u64` start offset + `u64` end offset. Files are strictly increasing by id,
sites strictly increasing by subject, every site references a file row whose
byte length covers its span, and every subject names an identity that occurs
in the module. Decoding rejects trailing bytes and requires byte-for-byte
re-encoding.

<!-- debug-source-origin-tags -->
| Tag | Source origin |
| --- | --- |
| 1 | User |
| 2 | Toolchain |

<!-- debug-subject-tags -->
| Tag | Debug subject | Id fields |
| --- | --- | --- |
| 1 | Machine | machine id |
| 2 | Block | block id |
| 3 | Operation | operation id |
| 4 | Edge | edge id |
| 5 | Value | value id |
| 6 | Contract | contract id |
| 7 | Obligation | obligation id |
| 8 | Place | place id |
| 9 | Claim | machine id + claim id |

## Optimization execution

The record is `PSIOEXE\0` + `u16` format marker 1 + `u64` selection byte
length + the selection bytes + input terminal identity + 32-byte input proof
fingerprint + output terminal identity + 32-byte output proof fingerprint. A
terminal identity is `u16` vocabulary marker + 32-byte program fingerprint.
Decoding rejects trailing bytes, requires the empty selection set to leave
both input/output identities unchanged, and requires byte-for-byte
re-encoding.

## PCC proof sidecar

A proof-carrying-code sidecar is `PCCPROOF` + `u16` format marker 1 + `u8`
product kind + the 32-byte artifact commitment + string semantic profile +
string checker profile + counted guarantees + `u64` evidence length +
evidence bytes + counted assumption strings + counted dependencies + no
trailing bytes. A guarantee is a string identity + counted premise strings; a
dependency is a string identity + a 32-byte content commitment. The guarantee
set is nonempty; guarantees, assumptions, and dependencies are canonically
ordered, and the decoded value re-encodes byte-for-byte. [PCC
publication](../proofs/publication.md) owns the semantic contract these
identities certify.

<!-- pcc-product-kind-tags -->
| Tag | Product |
| --- | --- |
| 1 | Psi |
| 2 | Native |

## Mathematical certificate

A mathematical certificate is `PSICORE\0` + `u16` format marker 3 + `u32`
level arity + the counted node table + the counted declaration signature +
the counted context + two `u32` root handles. It is the self-contained
judgment a receiver replays in the kernel: declaration statements and bodies,
context bindings, and the claimed `term : expected` pair all name rows in one
shared term table.

<!-- certificate-framing -->
| # | Framing field | Bytes |
| --- | --- | --- |
| 1 | level arity | `u32` level parameter count of the judgment |
| 2 | term table | `u32` node count + concatenated node bytes |
| 3 | declaration signature | counted rows: `u32` level arity + `u32` statement handle + `u8` body presence (1 + `u32` body handle, or 0) |
| 4 | context | counted `u32` binding handles |
| 5 | judgment roots | `u32` term handle + `u32` expected-type handle |

Every `u32` field in a node row is a handle into earlier rows: the table is a
postorder DAG whose nodes are keyed by their own wire bytes, so two encoders
of the same judgment emit the same table and producer-local aliases collapse
onto one entry. Signature statements and bodies enter the table first, in
signature order, then the context, then the judgment roots. A `Dummy` term
reachable from any root rejects the certificate. Decoding bounds term nesting
and level nesting at 256 each, requires every child index to precede its
parent, rejects trailing bytes, and requires the decoded certificate to
re-encode byte-for-byte — unreachable nodes and alternate tables cannot alias
one certificate.

<!-- certificate-term-tags -->
| Tag | Term | Fields after the tag |
| --- | --- | --- |
| 1 | Variable | `u32` de Bruijn index |
| 2 | Sort | sort row |
| 3 | Pi | domain + codomain |
| 4 | Lambda | domain + body |
| 5 | Apply | function + argument |
| 6 | Sigma | domain + codomain |
| 7 | Pair | first + second |
| 8 | Fst | pair |
| 9 | Snd | pair |
| 10 | Two | — |
| 11 | TwoZero | — |
| 12 | TwoOne | — |
| 13 | CaseTwo | motive + zero branch + one branch + scrutinee |
| 14 | Id | ty + left + right |
| 15 | Refl | ty + value |
| 16 | IdElim | motive + base + endpoint + proof |
| 17 | W | carrier + children |
| 18 | Sup | carrier + children + label + function |
| 19 | IndW | motive + step + tree |
| 20 | Constant | `u32` declaration index + counted level arguments |
| 21 | Empty | — |
| 22 | EmptyElim | ty + scrutinee |
| 23 | Squash | ty |
| 24 | SquashIntro | ty + value |
| 25 | SquashElim | proposition + function + scrutinee |
| 26 | Box | ty |
| 27 | BoxIntro | ty + value |
| 28 | BoxElim | motive + body + scrutinee |

Unadorned names in the field column are `u32` handles into earlier table
rows. A sort row is one `u8` tag plus its level; a level is a small tree,
written inline with no sharing table.

<!-- certificate-sort-tags -->
| Tag | Sort | Fields after the tag |
| --- | --- | --- |
| 1 | Type | level |
| 2 | Strict | level |

<!-- certificate-level-tags -->
| Tag | Level | Fields after the tag |
| --- | --- | --- |
| 1 | Constant | `u32` value |
| 2 | Parameter | `u32` index |
| 3 | Successor | level |
| 4 | Maximum | level + level |

## Section identities

| Section | Identity and role |
| --- | --- |
| Semantic module | Domain-separated commitment to exact canonical bytes; excludes replaceable proof, installation, and debug evidence. |
| Proof bundle | Independently identified evidence, published only when requested in `<artifact>.proof`, binding the semantic artifact and exact proof profile/dependencies. The current bounded `PSIPRF\0\0` encoding is not the complete general sidecar schema. |
| Obligation ledger | Independently identified reconstructed-obligation binding to the semantic subject and trust graph; proof routes stay replaceable evidence. |
| Optimization execution | Selection/output semantic provenance is rejoined at decoding. Internal proof identities and portable preservation evidence remain distinct; absent PCC does not waive transformation checking. |
| Installation | Separate `PSIINST\0` bytes and identity, retaining realization evidence without granting admission. |
| Debug map | Replaceable presentation metadata bound to the exact semantic subject, never program meaning. |

Each codec-emitted envelope opens with an eight-byte magic and a `u16` format
marker; the semantic module, sealed proof section, obligation ledger, and
debug map envelopes then carry the shared `u16` vocabulary marker (107). A
receiver rejects an unknown magic or stale marker before reading any counted
table. The installation record `PSIINST\0` is emitted outside this codec.

<!-- envelope-markers -->
| Envelope | Magic | `u16` marker | Vocabulary field |
| --- | --- | --- | --- |
| semantic module | `PSITERM\0` | 105 | yes |
| proof bundle | `PSIPRF\0\0` | 33 | no |
| sealed proof section | `PSIPSC\0\0` | 1 | yes |
| obligation ledger | `PSIOBLG\0` | 3 | yes |
| canonical artifact | `PSIART\0\0` | 2 | no |
| PCC proof sidecar | `PCCPROOF` | 1 | no |
| debug map | `PSIDBG\0\0` | 1 | yes |
| optimization execution | `PSIOEXE\0` | 1 | no |
| mathematical certificate | `PSICORE\0` | 3 | no |

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
