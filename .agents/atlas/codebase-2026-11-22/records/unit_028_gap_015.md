# Record: unit 028, gap 015 (second pass)

Area `12-native-representations`. Checkout `C:/Development/Omega/.agents/worktree/worker-2/` at `4f3c255d`.
Unit file: `units/gap_015.tsv`, one row: `source-rust/subsystems/compiler/psi-to-native/representations/abstract-operations/optimization-unit/src/identity.rs` lines 4401-5662 (1262 lines). Paths below are relative to `.../optimization-unit/src/` and written `identity.rs:N`.

To place the range I also read (outside the range, for context only) `identity.rs:3437-3537` (the family router) and `identity.rs:4272-4400` (start of `scalar::encode`), plus the `encode_node` call site `identity.rs:3215-3226`.

## 1. identity.rs 4401-5512: tail of `operation_encoding` (scalar, scalar_shapes, structural)

### Purpose
This code turns each `AbstractOperation` into a deterministic byte string. The bytes are hashed into the optimization unit's identity (`OptimizationUnitIdentity`, see header comment `identity.rs:1-8`), so any change to an operation's fields changes the identity. The range holds the end of the `scalar` encoder, the helper module `scalar_shapes`, and the whole `structural` encoder.

### Shape
- `operation_encoding::encode_operation` (`identity.rs:3440`, read) routes by operation variant to `structural::encode`, `atomic::encode`, `calls_and_effects::encode`, `ieee_float::encode`, `scalar::encode`, `control::encode`. Its only caller is `encode_node` at `identity.rs:3216` (grepped: one call site for the router).
- `scalar::encode` (`identity.rs:4291`, range 4401-4739 is its second half). Arms in this range: `ByteSequencePredicate` (tag 100), `ByteSequenceEqual` (104), `StructuralByteSequenceFieldRead` (89), `StructuralByteSequenceFieldLength` (76), `ByteSequenceLength` (62), `ElementViewLength` (86), `ElementViewRead` (87), `ElementViewWrite` (88), `IntegerConstant` (9), `BooleanConstant` (10), `StructuralScalarFieldRead` (11 for Boolean results, else 51), `BooleanNot` (12), `BooleanEqual` (13), `IntegerEqual` (14), `IntegerLessThan` (15), `IntegerLessOrEqual` (16), `IntegerBitwiseNot` (17), `IntegerWiden` (18), `ScalarRepresentationView` (99), `IntegerExactCast` (19), `IntegerBitwiseAnd/Or/Xor` (20/21/22), `IntegerShift` (23-26 via `integer_shift_tag`), `IntegerArithmetic` (27-41 via `integer_arithmetic_tag`), `TrappingInteger` (90).
- `integer_shift_tag` (`identity.rs:4741`): 2x2 match on `(IntegerShiftPolicy, IntegerShiftDirection)`: Wrapping Left 23, Wrapping Right 24, Exact Left 25, Exact Right 26.
- `integer_arithmetic_tag` (`identity.rs:4750`): exhaustive match over `(policy, operator)`, 15 pairs, tags 27 to 41. It includes pairs such as `(Saturating, Divide)` 39 and `(Saturating, Remainder)` 40 (`identity.rs:4766-4767`), so the identity code gives tags to combinations; whether the type can build them is not decided here.
- `encode_byte_sequence_operand` (`identity.rs:4772`): `ByteSequenceOperand::Whole` writes `0`, source argument, length; `Field` writes `1`, source argument, field id, length id. Used at `identity.rs:4403, 4415, 4416` and from the structural module at `identity.rs:5056` (grepped, 4 call sites in the file).
- `scalar_shapes` (`identity.rs:4796-4934`): `encode_untyped_unary` (tag, op, result, operand), `encode_untyped_binary`, `encode_typed_unary` (adds integer type), `encode_cast` (tag, op, optional obligation, result, source type, target type, operand), `encode_typed_binary` (tag, op, optional obligation, result, type, left, right), `encode_shift`, `encode_trapping` (writes a primitive code 0-7 then each operand id). Call counts read in `scalar::encode`: `encode_untyped_unary` 1, `encode_untyped_binary` 4, `encode_typed_unary` 1, `encode_cast` 2, `encode_typed_binary` 4 (three bitwise arms plus arithmetic), `encode_shift` 1, `encode_trapping` 1 (read; not grepped repo-wide because all are `pub(super)` inside this file's module).
- `structural::encode` (`identity.rs:4949-5466`), with local `tag!` macro (`identity.rs:4951-4956`) that writes the tag byte then `*psi_operation`. Arms: activation claims (253 establish, 254 release, 251 split, 252 merge), `EstablishNativeCodeLayout` 255, `ObserveStorageLocation` 122, `EstablishByteSequenceConcatenation` 92, `EstablishStructuralArray` 105, `EstablishScalarArray` 69, `EstablishPrimitiveLocal` 65, `PrimitiveLocalStore` 66, `StructuralCaseMembership` 71, `PrimitiveScalarRead` (67 when path empty, else 73), `ByteSequenceSubslice` 64, `ElementViewSubslice` 84, `EstablishElementView` 85, `WriteOnlyPrimitiveStore` (106 / 49 / 74), `StructuralScalarFieldStore` (75 / 50), `EstablishCase` 48, `EstablishByteSequenceLiteral` 1, `EstablishTrivialAffineLocal` 2, `EstablishZeroedStructural` 90, `EstablishStructuralLocal` 103, `EstablishRecord` 56, `EstablishAccessReference` 113, `EstablishReference` (110 with storage, else 79), `ReleaseReference` 80, `MoveStructuralField` 81, `StoreStructuralField` (82 Vacant, 117 ReplaceDiscardable), `TransformContent` 124, `DecomposeStructuralValue` 138, `MoveStructuralValue` 123, `StructuralLeafCopy` (88 Ordinary path, 91 Case path).
- `encode_store_destination` (`identity.rs:5468`): `Parameter` 0, `OwnedResult` 1, `OwnedBlockParameter` 2. Two callers (`identity.rs:5208, 5227`).
- `encode_record_initializer` (`identity.rs:5491`): field id, then `Scalar` 0 (value id, optional range obligation) or `Structural` 1 (argument). Two callers (`identity.rs:5244, 5290`).

### Inputs and outputs
Reads an `&AbstractOperation` (from crate `abstract_operations`) and appends to a `&mut CanonicalBytes` (the enum at `identity.rs:2934`, which can collect, count length, or feed a digest). Nothing is returned; no other state is touched. Helpers come from `carrier_encoding`, `structural_encoding`, `proposition_encoding` in the same file.

### How it decides
- Which byte layout an operation gets is keyed on the enum variant, and in several variants on a field value as well: result scalar type being `Boolean` (`identity.rs:4542`), `path.is_empty()` (`identity.rs:5117`, `5202`), `range_obligation.is_some()` (`identity.rs:5200`, `5225`), `storage.is_some()` (`identity.rs:5355`), `disposition` (`identity.rs:5400-5403`), `StructuralCopyPath` variant (`identity.rs:5448`). All are identities or structure, none is a display name or string compare.
- `ElementViewRead` (`identity.rs:4484-4489`): if `source.path` is empty it writes the place id alone; otherwise it writes `u64(0)` then the whole argument. The two forms differ only by the leading `0`; this relies on a place id never being 0 (inferred: not checked here).
- `ElementViewRead` and `ElementViewWrite` put the `path` field in different positions (last at `identity.rs:4493`, first at `identity.rs:4505`).
- `StructuralScalarFieldRead` writes the Boolean form with only `result.value` and no scalar type (`identity.rs:4542-4545`), the non-Boolean form through `encode_abstract_result` (`identity.rs:4547-4549`).
- `IntegerConstant` and `BooleanConstant` write `*result` as a bare id (`identity.rs:4521`, `4532`), while neighbouring arms use `encode_abstract_result` (`identity.rs:4402`).

### Cases by shape
- The router lists variants with macro patterns (`abstract_operations::byte_len_or_field_read_pattern!` and similar, `identity.rs:3495-3510`); each sub-encoder ends with `_ => unreachable!("operation family routing admitted a non-scalar operation")` (`identity.rs:4737`) or `"... non-structural operation"` (`identity.rs:5464`). A variant added to the router but not to the sub-encoder panics at run time rather than at compile time (read).
- `integer_arithmetic_tag` and `integer_shift_tag` are exhaustive over the two enums, so a new operator or policy fails to compile (`identity.rs:4741-4769`).
- `encode_trapping` maps `TrappingIntegerPrimitive` to codes 0-7 with an exhaustive match (`identity.rs:4920-4929`).

### Same job elsewhere
- The same "write a tag byte, psi operation id, then fields" job is done for the other families in the same module: `atomic::encode` (`identity.rs:3550`), `calls_and_effects::encode` (`identity.rs:3738`), `control::encode` (`identity.rs:4006`), `ieee_float::encode` (`identity.rs:4188`) (read at the signature level only; bodies outside my range).
- Tag byte reuse across families (read; `encode_node` writes no family byte before the operation, `identity.rs:3215-3217`):
  - tag 48: `control` `StructuralCase` (`identity.rs:4058`, starts with `psi_block` id) and `structural` `EstablishCase` (`identity.rs:5241`, starts with `psi_operation` id).
  - tag 88: `scalar` `ElementViewWrite` (`identity.rs:4504`) and `structural` `StructuralLeafCopy` with `Ordinary` path (`identity.rs:5449`).
  - tag 90: `scalar` `TrappingInteger` (`identity.rs:4730`) and `structural` `EstablishZeroedStructural` (`identity.rs:5273`).
  I checked these by grepping for the specific tag numbers; I did not grep every tag in the file, so other collisions are possible (suspected). Whether the bytes after the tag can still collide depends on the id types (block id vs operation id), which I did not trace.
- The encoding of an integer-typed binary operation exists twice in shape: `encode_typed_binary` (`identity.rs:4864`) and `encode_shift` (`identity.rs:4884`) differ only by the second type field.
- A second, separate encoding of the same scalar types for proofs/ranges exists at `crate::range::encode_integer_type` (used by the test at `identity.rs:5526`); outside my range, not read.

### Unused or one-caller code
- All helpers in this range are `pub(super)` or private inside nested modules, so I counted call sites by reading the enclosing function: counts are in Shape above. `encode_untyped_unary`, `encode_typed_unary`, `encode_shift`, `encode_trapping` each have one caller. None has zero.
- `encode_byte_sequence_operand` is `pub(super)` in `scalar` and imported by `structural` (`identity.rs:4946`), so it is the shared helper for that operand type: 4 call sites in this file (grepped on the name).
- No comment in this range calls anything the single or canonical way.

### Stated limits
None in this range. The only comments are `#[allow(clippy::too_many_arguments)]` on `encode_cast`, `encode_typed_binary`, `encode_shift` (`identity.rs:4843`, `4863`, `4883`).

### Fail-open risks
none seen. The wildcard arms panic (`unreachable!`, `identity.rs:4737`, `5464`) instead of skipping an operation. The one place that could make two different operations hash equal is tag reuse (above): classified "open" (not traced), and the consequence would be two different optimization units sharing an identity only if every following byte also matched.

## 2. identity.rs 5514-5551: `width_identity_tests`

### Purpose
One unit test that pins the exact bytes of a fixed integer width and checks that rigid parameterized widths with different owner machine or value id encode differently.

### Shape
`width_identities_keep_fixed_bytes_and_distinguish_rigid_parameter_owners` (`identity.rs:5531`). Helpers `identity` (`identity.rs:5518`, uses `carrier_encoding::encode_integer_type`) and `range_identity` (`identity.rs:5524`, uses `crate::range::encode_integer_type`).

### Inputs and outputs
Builds `IntegerType` values (signed 32 fixed; `parameterized` with `MachineId` and `ValueId`). Asserts `identity(fixed) == [2, 32, 0]` and `range_identity(fixed) == [1, 1, 32, 0]` (`identity.rs:5533-5534`), that three variants of the parameterized type differ in both encodings (`identity.rs:5545-5548`), and that `ClosedIntegerType::try_from` rejects the parameterized type (`identity.rs:5549`).

### How it decides / cases / limits / fail-open
Compares byte vectors. Fixed bytes are pinned literally. Fail-open: none seen (test code).

## 3. identity.rs 5553-5662: `float_meaning_identity_tests`

### Purpose
Two tests that the byte identity of a `Proposition::FloatMeaningEqual` clause changes when any source term, contract commitment, operand order, or side changes, and when an integer operand's source changes.

### Shape
- `bytes` helper (`identity.rs:5564`): encodes through `proposition_encoding::encode_proposition`.
- `float_meaning_identity_keeps_sources_catalogs_and_ordered_operands` (`identity.rs:5574`): builds a Binary32 `subtract` clause from `FloatSemanticOperation::from_source_identity("FloatSemantics", "subtract", ...)` (`identity.rs:5578-5584`). For each clause value position it mutates the source (Projection), the contract commitment byte 0 (both kinds), swaps operands 1 and 2 (Application), and asserts the bytes differ from the original (`identity.rs:5593-5619`). Then swaps `left` and `right` and asserts a difference (`identity.rs:5621-5623`).
- `float_meaning_identity_keeps_integer_operands` (`identity.rs:5627`): builds a `from_integer` clause on Binary64 with an `i32` operand and uses `map_sources` to replace the integer source with a different value id, asserting the bytes differ (`identity.rs:5651-5660`).

### How it decides / cases / limits / fail-open
Checks inequality of encodings only. In the first test the `Projection` arm asserts after changing `source` (`identity.rs:5598`) but the later mutation of `contract.commitment[0]` in that arm (`identity.rs:5605`) is checked only by the shared assert at `identity.rs:5619`; the `Application` arm checks the commitment change at `identity.rs:5609` and the operand swap at `identity.rs:5619` (read). The operation names are source strings ("FloatSemantics", "subtract", "from_integer") passed to a constructor, not decided on here. Fail-open: none seen (test code).

## Unused or one-caller code (whole unit)
Greps run: names inside `operation_encoding` submodules are not reachable outside `identity.rs`, so I did not run repo-wide greps for them. I ran `Grep encode_operation\(|encode_optional_field|encode_byte_sequence_operand|scalar_shapes::|fn mask_bit|fn obligation` over `source-rust/subsystems/compiler`: the router `operation_encoding::encode_operation` has 1 caller (`identity.rs:3216`); `encode_optional_field` has 3 sites (`identity.rs:4358, 5390, 5407`); `encode_byte_sequence_operand` 4 call sites plus definition. `mask_bit` is defined in `language-core/src/byte_predicates.rs:47` and used here at `identity.rs:4404`.

## Coverage
- Read in full: `identity.rs` lines 4401-5662 (1262 lines), the whole unit.
- Read for context outside the unit: `identity.rs` 3213-3227, 3437-3537, 4048-4062, 4272-4400, and lines 1-42 (header and imports).
- Skimmed: none. Not read: none.

## Five things a maintainer most needs to know about this unit
1. `identity.rs:4401-5512` defines the bytes that go into the optimization-unit identity; any change to a tag number, field order or a conditional layout (e.g. `identity.rs:4542`, `5117`, `5200`) changes every identity, and nothing in the range versions the layout.
2. The tag byte is not unique across families: 48, 88 and 90 are each used by two different operations (`identity.rs:4058`/`5241`, `4504`/`5449`, `4730`/`5273`), and `encode_node` adds no family prefix (`identity.rs:3216`). I grepped only those numbers.
3. Router and encoders are two separate lists; a variant missing from a sub-encoder panics via `unreachable!` (`identity.rs:4737`, `5464`) instead of failing to compile.
4. Several arms use layouts that depend on a value: `ElementViewRead` writes either the place id or `0` plus an argument (`identity.rs:4484-4489`); `PrimitiveScalarRead`, `WriteOnlyPrimitiveStore`, `EstablishReference` and `StructuralScalarFieldRead` change tag by field state.
5. Tests in the range pin only the width encoding (`[2,32,0]` and `[1,1,32,0]`, `identity.rs:5533-5534`) and inequality for float-meaning clauses; none of the 60-odd operation arms in this range has a byte-level test here (grepped: 3 `#[test]` in the range, read).

## What you are unsure about
- Whether the three shared tags (48, 88, 90) can produce equal full encodings; needs the id types and prefix rules traced (not done).
- Whether other tag numbers are shared across families; I checked only a list of scalar and structural tags, not all of `atomic`, `calls_and_effects`, `control`, `ieee_float`.
- Whether `(Saturating, Divide)`, `(Saturating, Remainder)` and similar pairs are constructible by `IntegerArithmetic`; this code gives them tags regardless.
- I did not confirm the number 60 in item 5 with a command; it is a rough count of arms, not a measured figure.
