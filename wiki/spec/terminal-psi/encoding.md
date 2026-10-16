# Terminal canonical encoding and identities

[Portable product](product.md) | [Observations](observations.md)

This is the canonical encoding contract, not yet a complete byte-level decoder
specification. The [documentation index](../../README.md) lists operation subjects.
[Proof values](mathematical_values.md) and [certificate rules](integer_certificates.md)
define semantics; the tables below give the physical layout of every operation,
terminator, scalar-term, and proposition form the codec accepts, plus the
catalog, scalar-block-invariant, and machine rows.

## Canonical form

Semantic bytes begin with `PSITERM\0`. They use fixed-width little-endian counts,
stable nonzero identities, full-width integer payloads, and explicit closed-sum
tags. Unordered sets are strictly sorted by stable identity or canonical bytes
and reject duplicates; symmetric terms use the same ordering. Parameters,
operations, jump arguments, and other execution-significant sequences retain
their declared order. Recursive terms have a fixed supported depth bound.

Decode rejects stale markers, unknown tags, invalid identities/values,
noncanonical order/forms, structurally invalid modules, truncation, and trailing
bytes. Successful decoding re-encodes byte-for-byte; it never normalizes an
alternative spelling. The pre-release contract has no implicit migration:
semantic changes move producer, codec, interpreter, and lowerers
together. Current golden bytes establish the current format, not historical
acceptance.

[Static machines](../language/machines.md#static-declarations-and-explicit-state)
use ordinary explicit parameter positions and identities. Structural parameters,
provider signature parameters, structural places and retained-borrow parameter
roots carry no `self` flag. Removing that byte changes the canonical format;
old encodings reject through the stale-marker rule rather than being accepted
as a compatibility dialect. Access, multiplicity, qualifications and custody
remain independently checked for every ordinary parameter.

`STATIC-MACHINE-MIGRATION` tracks completion of the coordinated producer,
codec and consumer migration. These byte layouts do not claim that
the remaining consumer migration or end-to-end validation is complete.

The same distinction applies to the legacy conformance subject field. The
[named-implementation contract](../language/conformances.md#data-and-code-are-independent)
has no subject. The [dynamic contract](dynamic_dispatch.md) retains an
implementation selector separately from zero or more ordinary call arguments.
`NAMED-IMPLEMENTATION-CONFORMANCE-MIGRATION` must remove the legacy subject field
across producers, codecs, interpreters and consumers together. The retired
data-attached dynamic operations, catalog sections and suspension targets have
no wire form: their former operation tags 48, 49, 52, 53 and 54 and suspension
target tags 3 and 4 are unassigned and reject, and
`DYNAMIC-IMPLEMENTATION-SELECTION-MIGRATION` adds the selector route's own rows.
The byte tables record the current implementation during that migration; they
do not authorize retaining the old semantic model or decoding it as a
compatibility dialect.

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
roster participate in semantic identity. The block-predicate representation
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
| structural path | counted segments, each `u8`: 1 + `u32` interned field identity; 2 + `u64` fixed index; 3 referent; 4 + `u64` start + `u64` end for a fixed byte range; 6 + value id + obligation id for a runtime index; 7 + case id; 8 + structural type id + `bool` scalar-codec presence + optional scalar type for a representation view; retired tag 5 rejects |
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
| structural parameter | place id + `u32` position + structural type id + multiplicity + access + counted domain ids + counted projected qualifications |
| scalar term list | counted scalar terms |
| proof term list | counted proof terms |
| obligation id list | counted obligation ids |
| claim transfer | claim id + `u32` argument index + structural path relative to the selected argument; Case segments retain conditional payload guards |
| completion receipt | claim id + `u32` argument index + structural path relative to the selected argument |
| crash routes | counted buckets, each `u8` cause (1 Trap, 2 Abort) + counted guards; each guard `u8`: 0 truth, 1 + proposition |
| successor edge | edge id + target block id + counted value ids + scalar term list + proof term list + counted structural arguments + counted trivial discards |
| trivial discard | place id; appears only inside a counted roster |
| residual discard | place id + structural path + structural type id |
| affine cleanup action | `u8`: 1 + place id; 2 + residual discard; 3 + place id + structural path + structural type id + machine id + counted claim ids + optional identity + obligation id list; 4 + place id |
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

A scalar term is a `u8` form tag followed by its fields.

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
| 36 | ViewExtent | root place id + counted canonical path segments + integer type |
| 37 | IeeeFloatLiteral | IEEE format tag + exact format-width bits |
| 38 | IeeeFloatField | root place id + counted canonical path segments + IEEE format |
| 39 | ByteSequencePredicate | predicate `u8` + root place id + counted canonical path segments |
| 40 | StructuralRead | root place id + scalar type + counted scalar-read projections |

The byte-predicate tag selects exactly one primitive: `1` is `valid_utf8`, `2`
is `no_nul`, `4` is `ascii_only`, and `8` is `non_empty`. Other values reject.
An empty path observes the whole byte descriptor; a nonempty path observes its
exact byte field. The term has Boolean type and supplies no assertion that its
predicate holds.


A scalar-read projection is a tag followed by its payload: `1` selects a field
id, `2` a fixed `u64` index, `3` a case id, and `4` a recursively encoded scalar
index term. The path is nonempty. Dynamic indices keep their own fixed-integer
carriers, excluding addresses; they do not become static geometry. The exact
root declaration and each selected member or element determine the leaf type.
Selector terms participate in ordinary depth limits, value substitution and
structural-root substitution. A term supplies no bounds, access or truth evidence.


## Integer math terms

The proof-only unbounded integer syntax.

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
bound. A Scalar leaf embeds a scalar term; it records one scalar field of a
record/enum carrier construction and never stands in for a runtime operand.

<!-- proof-term-tags -->
| Tag | Proof term | Fields after the tag |
| --- | --- | --- |
| 1 | Construction | type identity string + `bool` case-identity flag (+ case identity string when set) + counted fields (field identity string + proof term) |
| 2 | Formal | `u32` position |
| 3 | Scalar | scalar term |

## Content terms and places

The conservation algebra and content-place grammar used by proposition 9.

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

A proposition is a `u8` form tag followed by its fields.

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
| 18 | StructuralIdentity | structural type id + canonical structural field + structural type id + canonical structural field |

Each `StructuralIdentity` subject denotes the complete value at its path in
the enclosing predicate's invocation frame. The two endpoint types must match
exactly; operands use canonical subject order. This encoding retains contextual
data identity without enumerating fields or array elements and does not encode
an executable equality operation.

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
| 35 | BoundaryCall | boundary machine id + counted value ids + counted structural arguments + counted completion receipts + counted requirement obligation ids |
| 36 | PortWrite | service id + `u16` port + `u8` value |
| 37 | EstablishTrivialAffineLocal | place id |
| 38 | BooleanStructuralField | source place id + scalar field carrier path + field id |
| 39 | CallStructuralScalar | callee machine id + counted value ids + scalar term list + proof term list + counted structural arguments + counted claim transfers + obligation id list + crash routes |
| 40 | EstablishByteSequenceLiteral | destination place id + counted bytes |
| 41 | CallStructural | callee machine id + counted structural arguments + counted claim transfers + obligation id list + crash routes + counted selected evidence bindings |
| 42 | EstablishScalarCase | case id + counted fields (field id + value id + optional obligation id) |
| 43 | WriteOnlyPrimitiveStore | destination place id + value id; root-only form |
| 44 | IeeeFloatConstant | IEEE float value |
| 45 | NearestIeeeFloatFusedMultiplyAdd | left value id + right value id + addend value id |
| 46 | StructuralScalarFieldStore | destination place id + structural path + field id + value id; unchecked form |
| 47 | ScalarStructuralField | source place id + scalar field carrier path + field id |
| 50 | CallStructuralWithScalarArguments | callee machine id + counted value ids + scalar term list + proof term list + counted structural arguments + counted claim transfers + obligation id list + crash routes |
| 55 | retired | whole-place ByteSequenceLength payload is rejected |
| 56 | ByteSequenceRead | source place id + index value id + length value id + obligation id |
| 57 | retired | whole-place ByteSequenceSubslice payload is rejected |
| 58 | StructuralByteSequenceFieldStore | destination place id + structural path + field id + source place id + length value id + obligation id |
| 59 | StructuralByteSequenceFieldLength | source place id + structural path + field id |
| 60 | StructuralByteSequenceFieldByteStore | destination place id + structural path + field id + index value id + value id + length value id + obligation id |
| 61 | retired | former primitive establishment without receiving evidence |
| 117 | EstablishPrimitiveLocal | value id, optional receiving obligation id |
| 62 | PrimitiveScalarRead | source place id; root-only form |
| 63 | ByteSequenceWrite | destination place id + index value id + value id + length value id + obligation id |
| 64 | EstablishScalarArray | counted value ids |
| 65 | IeeeFloatCompare | IEEE float relation + left value id + right value id |
| 66 | StructuralCaseMembership | source place id + structural path + case id |
| 68 | EstablishRecord | counted fields (field id + `u8` operand tag: 1 + value id + optional obligation id; 2 + structural argument) |
| 69 | EstablishReference | structural argument |
| 70 | ReleaseReference | place id |
| 75 | StructuralScalarFieldStore | destination place id + structural path + field id + value id + obligation id; range-checked form |
| 77 | MoveStructuralField | source place id + structural path + field id |
| 78 | StoreStructuralField | destination place id + structural path + field id + structural argument |
| 118 | StoreStructuralField (intact discardable replacement) | destination place id + structural path + field id + structural argument |
| 79 | EstablishElementView | destination place id + source structural argument + element type id + element-view region |
| 98 | EstablishMutableElementView | destination place id + source structural argument + element type id + element-view region |
| 80 | retired | rejected; old whole-place element-length payload |
| 81 | ElementViewRead | source place id + index value id + length value id + obligation id |
| 82 | retired | whole-place ElementViewSubslice payload is rejected |
| 83 | EstablishStructuralCase | structural case id + counted fields (field id + `u8` operand tag: 1 + value id + optional obligation id; 2 + structural argument) |
| 84 | StructuralLeafCopy | source place id + structural path |
| 85 | StructuralCaseLeafCopy | canonical structural field (source place id + canonical path whose `Case` steps name the case the authorizing edge selected) |
| 86 | StructuralByteSequenceFieldRead | source place id + structural path + field id + index value id + length value id + obligation id |
| 89 | PrimitiveScalarRead | source place id + structural path; projected form requires a nonempty path, whose elements may be `RuntimeIndex` |
| 90 | WriteOnlyPrimitiveStore | destination place id + structural path + value id; projected form requires a nonempty path, whose elements may be `RuntimeIndex` |
| 91 | TrappingInteger | `u8` primitive (1 add, 2 subtract, 3 multiply, 4 divide, 5 remainder, 6 shift left, 7 shift right, 8 convert; others reject) + its operand value ids: left + right, value + count, or the one conversion operand. No cause, guard, or frontier follows: the operation is its own `Trap` site |
| 92 | ElementViewRead | source place id + index value id + length value id + obligation id + structural element path; the projected form requires a nonempty static path to a scalar leaf of the element (a record field or literal index; tag 81 spells the pathless read of a scalar element) |
| 93 | AtomicAccess | place id + structural carrier path + field id + `u8` event (1 load + ordering; 2 store + value id + ordering; 3 read-modify-write + `u8` operation (1 add, 2 subtract, 3 and, 4 or, 5 xor) + operand value id + ordering; 4 swap + value id + ordering; 5 compare-exchange + expected value id + replacement value id + success ordering + failure ordering; others reject). Each ordering is a `u8` (1 NoOrdering, 2 Receive, 3 Publish, 4 ReceivePublish, 5 GlobalOrder; others reject). Ordering legality, the static carrier path, field type, and root authority are verification's judgments |
| 100 | StructuralByteViewFieldStore | destination place id + structural path + field id (zero: the whole destination) + source place id; no predicate obligation |
| 105 | ByteSequenceEqual | left byte-sequence operand + right byte-sequence operand |
| 106 | StructuralByteViewFieldStore | destination place id + structural path + field id (zero: the whole destination) + source place id + predicate obligation id |
| 112 | ElementViewLength | source place id + structural access + structural path |
| 113 | ByteSequenceSubslice | source place id + structural access + structural path + start value id + end value id + length value id + obligation id |
| 114 | ElementViewSubslice | source place id + structural access + structural path + start value id + end value id + length value id + obligation id |
| 115 | ByteSequenceLength | source place id + structural access + structural path |

A byte-sequence operand is `u8` 2 + whole source structural argument + length value id,
or `u8` 1 + source place id + structural access + structural path + field id
+ length value id. Tag 0's retired whole-place operand rejects. Whole operands
retain the same place, access and path as their exact length observation; no
projection is inferred from the root's declaration. Other operand tags reject.
A view-field store without a
predicate obligation retains tag 100's existing payload; tag 106 carries the
additional exact-source obligation. Its decoder never infers a proof from an
unqualified store or interprets equality's tag 105 as a field store.

Tag 80's retired whole-place element-length payload rejects; tag 112 retains
the complete operand without reinterpreting old source identities.
Tags 51 (retired literal field row) and 67 (retired scalar-only record
operand) reject; their payloads are not reinterpreted as current forms. Tags
73, 74, 76 and 87 (retired canonical-path primitive read and store and the
runtime-indexed store and read, now tags 89 and 90 with a `RuntimeIndex`
path segment) reject the same way. Tags 71 and 72 are unassigned. Every other tag absent from the table rejects as an
unknown operation tag. The tag-41 selected evidence binding is: result type
id + result case id + `u32` guarded position + callee obligation id + callee
term id + output field string + callee proposition id + instantiated
proposition id + output evidence term id + result substitution (`u8`: 0
absent; 1 + `u32` argument position + callee result place id + caller result
place id) + counted argument substitutions (`u32` proposition argument
position + `u32` callee scalar parameter position, strictly increasing by
argument position) + validity record (result place id + counted proposition-dependency
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
| 18 | ReturnStructural | edge id + source place id + counted bindings (claim id + structural path relative to the returned value) + counted trivial discards |
| 7 | ReturnUnitPartialAffine | edge id + counted trivial discards + counted residual discards |
| 8 | ReturnUnitNominalAffine | edge id + counted cleanups (place id + structural type id + cleanup machine id + optional receiver place id + obligation id list) |
| 16 | StructuralCase | source place id + structural subject path + counted case successors (edge id + target block id + case id + counted field ids + counted affine cleanup actions) |

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
| 3 | InvokeNominal | place id + structural path + structural type id + machine id + counted claim ids + optional identity + obligation id list |
| 4 | RetireHome | place id |

## Machine rows

A boundary declaration stores its counted runtime parameter-order tags after
the optional attachment and before scalar parameter types: `u8` 0 selects the
next scalar, 1 the next structural parameter. Unknown tags, missing entries, and
lane-count mismatches reject. Reordering a valid roster changes the semantic
identity even when the parameters have identical physical shapes.

An element-view region is a required `u8` tag: `0` means the whole
collection and carries no payload; `1` means a representation byte
region and carries start value id, end value id, and obligation id, in that order.
Unknown tags and truncated payloads reject. The source structural argument
encodes place id, structural access, and counted structural path before the
element type id. Neither bounds nor target stride is a producer-supplied wire
constant; the verifier reconstructs them from the exact source and element.

`ElementViewWrite` encodes the destination place, counted structural path,
index value, stored value, length value, and obligation id, in that order.
An empty path denotes the scalar element itself. The path is part of semantic
identity and independently selects the destination leaf; its bounded carrier
contributes range clauses to the operation's index obligation.

Module bytes are `PSITERM\0` + `u16` format marker 151 + `u16` vocabulary
marker 123 + the entry machine id, followed by the module's counted tables in
the declaration order below and ending with boundary-application candidates.

<!-- module-table-order -->
| # | Module table | Row |
| --- | --- | --- |
| 1 | scalar qualification catalog | the five counted catalog tables above |
| 2 | structural types | counted structural type declarations |
| 3 | structural referent identities | counted identity-only pointer targets |
| 4 | structural domains | counted structural domain declarations |
| 5 | services | counted service declarations |
| 6 | concrete root service reach | counted service ids |
| 7 | installation reach dependencies | counted installation reach rows |
| 8 | placed-view inputs | counted placed-view input rows |
| 9 | reborrow restored call uses | counted restored-use rows |
| 10 | boundary machines | counted boundary machine declarations |
| 11 | provider candidates | counted provider candidate rows |
| 12 | dynamic dispatch | counted interfaces and implementation tables |
| 13 | float-meaning projections | counted projection rows |
| 14 | float-meaning equalities | counted equality rows |
| 15 | proposition declarations | counted proposition declaration rows |
| 16 | proposition applications | counted proposition application rows |
| 17 | evidence terms | counted evidence term rows |
| 18 | evidence contract lanes | counted evidence lane rows |
| 19 | proof-output invocations | counted proof-output call rows |
| 20 | proof recursive components | counted recursive component rows |
| 21 | closed conformance applications | counted conformance application rows |
| 22 | suspension rows | a bare `u32` call-plan count, then counted suspension call sites, then counted suspension call plans |
| 23 | scalar block invariants | the counted invariant rows above |
| 24 | operation crash contracts | counted operation crash contract rows |
| 25 | machines | counted machine rows |
| 26 | target-machine families | counted family rows |
| 27 | target layout applications | counted complete checked policy alternatives |
| 28 | boundary application demands | counted exact requirement/application rows |
| 29 | callback applications | counted checked registrar occurrences |
| 30 | boundary application candidates | counted checked bodies for closed applications |

A dynamic slot encodes its id, declaring-trait and requirement identity strings,
counted family tuple, counted scalar formals and counted structural formals.
Immediately after the structural formals, it encodes counted entry-claim paths:
each row is a structural-lane `u32` argument index followed by a counted static
structural path. Rows are strictly ordered by `(index, path)`, with no overlap
within an argument. They contain no implementation-local claim id. The optional
scalar result follows (presence `u8`, then scalar type). Every available table
row must realize the slot's signature and complete claim-path roster.

A boundary-application row carries its nonzero operation id, counted UTF-8
exact-owner declaration identity, counted overload identity, optional package
identity (presence Boolean then 32 nonzero digest bytes), and counted static
arguments. No arguments is the canonical empty application. Each argument has a
`u8` category and `u32` binder ordinal: tag `1` carries one exact-owner type
identity; tag `2` carries the declared carrier identity, value type identity and
canonical value encoding as counted UTF-8 strings. Identities and constant
encodings are nonempty. Rows are strictly ordered by operation id, each naming
one existing operation; arguments have contiguous zero-based binder ordinals.
The roster is semantic input, not a provider-selection or realization receipt.

A callback application carries a `u32` checked occurrence number, nonzero
registrar operation id, `u32` static-machine binder ordinal, counted UTF-8
canonical requirement identity, then the published 32-byte contract commitment
and selected checked-machine declaration. Rows are strictly ordered by occurrence number. Requirements and
commitments are nonempty and the registrar names an existing boundary call.
Several callback applications may share a registrar operation. Occurrence numbers
are local to the producing compilation and remain stable under pruning; they
are not source arena handles and alone authorize nothing.

A checked-machine declaration encodes exact-owner machine and initial-state
identity strings, a 32-byte contract commitment, then an optional specialization
(presence Boolean, template identity string, 32-byte commitment). Identities and
commitments are nonempty. Each machine envelope encodes an optional declaration
immediately after its machine id; synthetic bodies may omit it. A callback body
must retain it, and native admission compares it to the caller's selected body.

A candidate carries the same requirement and application encoding without an
operation id, followed by its selecting callable identity, optional selecting
package identity, optional target string, body and entry nominal identities,
32-byte contract commitment, and optional specialization (template nominal
identity plus 32-byte commitment). Optional fields use a presence Boolean.
Identities and commitments are nonempty; candidates are strictly ordered by
their complete record and duplicates reject. Repeated calls share this catalog;
target selection does not alter it.

A family row carries its canonical callable, a nonzero 32-byte checked head
contract commitment, and counted members ordered by the exact authored
deployment tag. A callable is a `u8` tag (`1` machine,
`2` boundary) followed by its nonzero id. Each member carries a counted UTF-8
deployment tag, callable, and a `u8` supply tag: `1` checked body, `2`
requirement, `3` top-level requirement, `4` boundary, `5` admission claim,
`6` external realization. Families are ordered by canonical callable; each
member callable belongs to exactly one member. An independent portable head
names a requirement boundary outside the member roster; a retained executable
canonical head instead names one of its members. Both forms retain the exact
checked head commitment, never a selected member's private contract substituted
for the portable head. Checked and opaque references must name their corresponding
module rosters. Deployment tags are source identities, not native ISA names.
Membership alone grants no call redirection or physical installation authority.
Provider-default declaration programs produce Build-owned selections rather
than executable members of this runtime roster.

Marker 138 adds the head commitment and completion-receipt path. A receipt's
path records current argument-relative custody, including conditional case
payload guards. Historical claim provenance remains separate: a claim moved
into a containing record need not still occupy its original entry path.
The call must settle the complete current owned roster at the selected paths;
the receipt neither recreates a claim nor grants boundary issuance. Earlier
markers are not reinterpreted under this layout.

Marker 140 adds target layout applications after target-machine families. A
record whose geometry remains target dependent uses structural shape tag `13`,
a nonzero layout-application `u32`, and its counted structural fields. It has
no packed or host-selected geometry in the portable artifact.

A layout application encodes its `u32` id, record and schema type ids, policy
identity string, family callable, and `u16` Schema ABI version. Its counted
Schema fields each carry record-field and schema-field ids, `u64` key, optional
`u64` identity, `u64` byte size and alignment, and a kind byte (`0` scalar,
`1` nested). Counted alternatives follow: deployment string, member machine id,
`u64` record size and alignment, then counted field ids with non-absent authored
field storage. The codec requires a complete exact family roster and validates
every alternative against the retained invariant Schema argument. This is
representation validation; semantic checking remains the producer's obligation.
Deployment selection consumes one retained alternative without changing the
canonical portable artifact or inferring default storage for an unresolved row.

The machine roster is strictly
ordered by machine id. A machine row is its machine id followed, in order, by
an optional attachment structural type identity, counted parameter value
declarations, counted structural parameters, a ranked-cycle byte, a machine
result row, counted structural place declarations, counted invocation entry
claims, counted block-entry claim bindings, counted loan completions, the
declared service reach and published service ceiling, a closed reach
application, counted content entry claims, counted content identity
reshuffles, counted content partition compositions, the entry block id,
counted block rows, and the machine contract.

| Machine row part | Fields |
| --- | --- |
| structural place declaration | place id + structural place kind |
| entry claim | claim id + input place id + structural path; strictly ordered by claim id |
| block-entry claim binding | claim id + owned block parameter place id + structural path; strictly ordered by claim id; names the arriving obligation without establishing one |
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
| 1 | Parameter | `u32` position |
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
| 1 | Parameter | `u32` position |
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
| structural type declaration | structural type id + identity string + no-code affine cleanup authority boolean + structural type shape |
| structural referent identity | structural type id + canonical closed-type identity string; no storage shape |
| structural field | structural field id + identity string + binding relevance + structural field type |
| structural case | structural case id + identity string + counted payload structural fields |
| structural domain declaration | domain id + semantic domain id + identity string + carrier structural type id + optional content projection + counted establishment routes |
| optional content projection | `u8` 0 absent; `u8` 1 + projection domain id + `u64` projection report fingerprint + content algebra + content projection expression |
| structural establishment route | `u8` tag per the establishment-route table |
| service declaration | service id + identity string + counted parent service ids |
| concrete root service reach | counted service ids |
| installation reach dependency | requirement identity string + counted service-id upper bound |

Storage declarations and identity-only referents share one type-id namespace.
Their ids and canonical identities are disjoint, and each roster is strictly
ordered by type id. Every identity-only row is the target of a reference
carrier. A pointer edge may name either roster; by-value fields, array elements,
owned storage and dereference projections require concrete declarations.
Naming a pointer target supplies no layout or access authority. This keeps
`Nest<T>` containing `&Nest<[T; 2]>` finite without truncating type identity or
manufacturing a placeholder shape. Zero array extent does not erase its element.

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
| 9 | retired | former Fused-only carrier; reject |
| 11 | BindingCarrier | no encoded payload; logical nonconstructible binding, no storage authorization |

The binding marker retains logical custody independently of provider selection.
It is not an empty record: record construction, zero initialization, and byte
observation cannot establish or inspect a binding. Existing values transfer or
borrow under their exact occurrence and requirement identity. This declaration
does not authorize payload erasure. Realization must establish each supplied
root and select its storage; the current native entry implementation supports
Fused establishment only. Independent storage cannot inherit that zero-payload
layout. Tag 9 is retired rather than reinterpreted as this weaker declaration.

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
| 7 | PredicateByteSequence | byte-sequence carrier + predicate-set bits |
| 8 | Atomic | scalar type; only Boolean and fixed-width, non-address integers are admitted |

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
| 6 | RuntimeIndex | selector value id + obligation id |

`RuntimeIndex` selects one element of the fixed array its prefix resolves to
by the runtime integer the selector value holds. The operation carrying the
path owns the obligation; the verifier reconstructs `selector < extent` (and
`0 <= selector` for a signed carrier) from the declared extent, so no bound is
encoded. Tag 5 carried the retired parameter-position spelling with a restated
interval and is refused.

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
| 5 | CapturedSelector | `u32` interned selector-formation identity |

<!-- restoration-class-tags -->

| tag | Reborrow restoration class |
| --- | --- |
| 1 | ExclusiveReactivation |
| 2 | SharedFreezeRestoration |

### Boundary machines and provider candidates

| Row | Fields |
| --- | --- |
| boundary machine | boundary machine id + identity string + declaring schema nominal identity + declaration-role tag + requirement-kind tag + completion tag + `bool` unconditional termination + optional attachment structural type id + counted parameter order + counted scalar parameter types + crash routes + counted structural parameters + result + counted structural requirements + counted scalar requires propositions + counted scalar ensures propositions + counted program-local root introductions + counted content guarantees + fixed service reach + optional owning service id + published service ceiling |
| optional structural type id | `u8` 0 absent; `u8` 1 + structural type id |
| structural requirement | `u32` argument index + domain id |
| program-local root introduction | `u32` argument index + `u32` source parameter position + qualification domain id + carrier structural type id + projection domain id + `u64` projection report fingerprint + content algebra + content projection expression + `u64` compatibility report identity |
| structural parameter | the machine-row structural parameter encoding |
| service reach | counted service ids |
| provider candidate | boundary machine id + requirement identity index + source requirement identity index + receiving application identity index + counted receiving region identity indices + provider identity index + selecting declaration nominal identity index + candidate identity index + source candidate identity index + optional source candidate package identity + candidate machine id + counted signature parameters + counted positional refinements + counted required domains + counted realized service-ceiling ids |
| optional source candidate package identity | `bool` present + 32-byte package digest when present |
| signature parameter | `u32` position + structural type id + structural multiplicity + structural access + counted qualification domain ids + counted projected qualifications |
| projected qualification | structural path + domain id |
| positional refinement | `u32` boundary parameter index + `u32` candidate parameter index |
| required domain | `u32` argument index + domain id |

Declaration-role tags are `0` for Callable and `1` for ContractEvidence.
Unknown tags reject. ContractEvidence retains proof-only foreign loan custody;
it cannot be invoked, installed, imported, or used as a target-family member.
Its schema owner introduces no runtime provider demand. Callable signatures
retain their exact declaring schema owner; provider candidates retain their
exact selecting declaration independently of display labels and callable bodies.

The receiving application retains the complete checked trait application, not
its erased runtime carrier. Receiving region identities follow the selecting
declaration's public binder order, including repeated and unused positions.
Zero application means no closed trait application; it is never a wildcard and
requires an empty region tuple. Nonzero application and region indices name
nonempty entries in this artifact's identity table. Only trait requirements may
carry this association. Installation preserves it beneath the exact admitted
candidate and artifact; retaining these bytes grants no selection, loan or
permission authority and does not itself open regional provider eligibility.

Boundary completion tags are `0` for the exact normal-return provider premise
and `1` for external process exit. Unknown tags reject. Completion is independent
of the result and reach rosters and must match the exact admitted provider
contract. The transitional external Unit encoding remains unsupported in
implicit cleanup and does not supply an external terminal invocation or trace.

The unconditional termination bit retains an explicit `Terminates` contract
with no progress premises. Normal completion alone does not promise finite
work. Conditional termination projects to false; its premises remain in the
provider schema and cannot be discharged by this bit. Native admission compares
the bit with that exact schema. Checked provider bodies and executable cleanup
dependencies must independently preserve finite normal completion, including
ranked control cycles and the contracts of opaque calls.

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
| 2 | Structural | structural type id + structural multiplicity + counted qualification domain ids + counted projected qualifications |

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
| 1 | Parameter | `u32` position + identity string |
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
crash routes, dense parameter positions, strictly ordered requirements,
canonical strictly increasing scalar requires propositions (the machine
contract `requires` rule) and strictly ordered service ceilings. Provider candidates are strictly ordered
by boundary machine id, then provider identity, then selecting declaration, then candidate identity, then
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

Implementation migration: the `optional subject identity` below is retired
semantic state. The replacement identity is the named implementation, its
complete telescope and trait application, rows, laws and callable identities.
Do not infer a subject from the first type argument or emit a permanent empty
subject field as the new representation.

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

### Suspension rows

The suspension section is a bare `u32` call-plan count, then counted call
sites, then counted call plans.

| Row | Fields |
| --- | --- |
| suspension call site | operation id + crossing id + suspension call target + 32-byte frontier commitment |
| suspension call plan | operation id + crossing id + suspension call target + carry policy + `u32` live value count + counted live values + loan graph |
| suspension live value | suspension place + suspension value type + suspension storage + `u32` claim count + counted claim ids + carry policy + value-loan tag and optional link span |
| carry policy | carry suspension + carry cpu + carry host thread + carry address (one tag byte each) |

<!-- suspension-target-tags -->

| tag | Suspension call target |
| --- | --- |
| 1 | Machine | machine id |
| 2 | Boundary | boundary machine id |

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
| 2 | Structural | place id + structural access + counted structural path segments |

<!-- suspension-value-type-tags -->

| tag | Suspension value type |
| --- | --- |
| 1 | Scalar | scalar type |
| 2 | Structural | structural type id |

<!-- suspension-storage-tags -->

| tag | Suspension storage |
| --- | --- |
| 2 | Parameter |
| 3 | Local |
| 4 | CallArgument |

Suspension call sites and plans are strictly ordered by operation id and
crossing id; each plan's live values are nondecreasing by place, storage,
value type, and carry policy. These are demand occurrences, not unique storage
identities: two actual positions may name the same storage and still contribute
separate rows, claims and policies. Preserve every occurrence and the complete
count; do not deduplicate equal coordinates. Each live value's claims are
strictly ordered by claim id. The frontier commitment uses the
`terminal-suspension-frontier-v7` domain and includes the ordered roster and
its cardinality, each value's loan links and the complete loan graph. Marker
137 adds evaluated-actual captures; earlier markers are not reinterpreted.

Each live value ends with a loan tag: `0` unresolved, `1` view-free, or `2`
followed by a span into the plan's loan-link arena. The plan then encodes counted
structural path segments, counted value-to-loan handles, counted supplier
handles and counted loan occurrences. Marker 158 replaces a single supplier
handle with a span of immediate supplier alternatives; earlier markers are not
reinterpreted. Shared predecessors retain custody without enumerating histories.
Handles are `u32` one-based slot plus `u32` generation: zero/zero is absent,
and resident slots use generation 1. Spans add a `u32` count and retain the
zero/zero/zero empty representation. Decoder bounds checks concern foreign
coordinates, not source borrow admission.

A loan occurrence encodes capture, backing, resource kind, parent handle,
supplier span, structural access and an active-at-call Boolean. Capture is
`0` unresolved, or `1` followed by formation operation, carrier place, source
place, source access and a source-path span, or `2` followed by a `u32`
call-argument live-value position and a captured-leaf path span. This position
must name a structural CallArgument occurrence. Its path selects the already
evaluated descriptor, not a source expression to evaluate again. Backing is
`0` unresolved,
`1` plus a `u32` retained-owner live-value position, or `2` plus an incoming
structural argument. Resource kind is `0` unresolved, `1` direct root, or
`2` reborrow. Parentage and supplier transport are distinct relationships.
Incoming referents are not descriptor-slot ownership; retired dependencies do
not become active because descendants retain them. These rows record demands,
not a completed lifetime, activation or physical-preservation judgment.

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

## Canonical artifact

The transport envelope is `PSIART\0\0` + `u16` format marker 3 + the counted
section framing below; a receiver rejects an unknown magic, a stale marker,
an unknown debug-presence tag, and trailing bytes.

<!-- artifact-framing -->
| # | Framing field | Bytes |
| --- | --- | --- |
| 1 | semantic section length | `u64` byte length of the semantic module section |
| 2 | optimization section length | `u64` byte length of the optimization execution section |
| 3 | debug presence | `u8`: 0 absent; 1 + `u64` debug section byte length |
| 4 | section bytes | semantic, optimization, then debug bytes when present, in that order |

Decoding is reconstruction: the semantic module validates on its own; the
optimization and debug sections rebind to it. The artifact manifest is rebuilt
from the section bytes, never transported. Every decoded section must re-encode
to the exact transported bytes — a producer's non-canonical serialization
rejects.

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

The record is `PSIOEXE\0` + `u16` format marker 2 + `u64` selection byte
length + the selection bytes + input terminal identity + output terminal
identity. A terminal identity is `u16` vocabulary marker + 32-byte program fingerprint.
Decoding rejects trailing bytes, requires the empty selection set to leave
both input/output identities unchanged, and requires byte-for-byte
re-encoding.

## Mathematical certificate

A mathematical certificate is `PSICORE\0` + `u16` format marker 3 + `u32`
level arity + the counted node table + the counted declaration signature +
the counted context + two `u32` root handles. It is the self-contained
judgment the kernel re-decides: declaration statements and bodies,
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
| Semantic module | Domain-separated commitment to exact canonical bytes; excludes installation and debug evidence. |
| Optimization execution | Selection/output semantic provenance is rejoined at decoding. |
| Installation | Separate `PSIINST\0` bytes and identity, retaining realization evidence without granting admission. |
| Debug map | Replaceable presentation metadata bound to the exact semantic subject, never program meaning. |

Each codec-emitted envelope opens with an eight-byte magic and a `u16` format
marker; the semantic module and debug map envelopes then carry the shared `u16`
vocabulary marker (122). A receiver rejects an unknown magic or stale marker
before reading any counted table. The installation record `PSIINST\0` is emitted
outside this codec.

<!-- envelope-markers -->
| Envelope | Magic | `u16` marker | Vocabulary field |
| --- | --- | --- | --- |
| semantic module | `PSITERM\0` | 151 | yes |
| canonical artifact | `PSIART\0\0` | 3 | no |
| debug map | `PSIDBG\0\0` | 1 | yes |
| optimization execution | `PSIOEXE\0` | 2 | no |
| mathematical certificate | `PSICORE\0` | 3 | no |

The reconstructed manifest binds each present component under its own hash domain.
Absent differs from present-but-empty. Replacing valid nonsemantic evidence
preserves semantic identity while changing its own identity.

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
