# Packet: unit_028_gap_015 (area 12, native representations)

Packet complete. Every claim in the [record](../../records/unit_028_gap_015.md) for its one range has a
disposition below. One claim was a real defect and is fixed in this PR: operations from different families
reused the same leading tag byte, so the optimization-unit identity did not say which operation a node held
(seven tags, not the three the record found). No claim stays open as a correctness defect; claim C11 stays
open as a test-coverage gap.

Scope: [unit](../../units/gap_015.tsv), one range:
`source-rust/subsystems/compiler/psi-to-native/representations/abstract-operations/optimization-unit/src/identity.rs`
lines 4401-5662 at the baseline. Below, `identity.rs:N` is a baseline line number (`24ad2700`), and the
range code is unchanged on `main` at `348268cb` apart from line shifts. Area 12 only.

Starting evidence: the [07-13 ledger](../07-13-psi-and-native.md) has no row for this range; the
[area 12 digest](../../digests/12-native-representations.md?plain=1#L212) line 212 repeats record claim C2.
Open PRs on 2026-11-24: none touches the operation encoder; #5432 and #5449 touch `identity.rs` and
`unit_validation.rs` for the separate `DeclarationIdentity` build break (see "Build state" below).

## Fixed

### C2. Operation tag bytes repeat across families (record §1 "Same job elsewhere", five-things item 2, "unsure" items 1-2; digest 12 line 212)
- Code: `operation_encoding::encode_operation` (`identity.rs:3440`) dispatched to six family encoders,
  and `encode_node` (`identity.rs:3216`) wrote no family byte first.
- Contract: the file header (`identity.rs:1-8`) says any structural or semantic drift produces a distinct
  identity. A tagged canonical encoding meets that only when the leading tag names the operation.
- Trace: a script listing the first tag written by each arm of the six family encoders found seven shared
  numbers, not three: 42 (`control` Jump `identity.rs:4035`, `ieee_float` IeeeFloatConstant `:4214`),
  43 (Conditional `:4047`, NearestIeeeFloatFusedMultiplyAdd `:4259`), 48 (StructuralCase `:4058`,
  EstablishCase `:5241`), 88 (ElementViewWrite `:4504`, StructuralLeafCopy Ordinary `:5449`),
  90 (TrappingInteger `:4730`, EstablishZeroedStructural `:5273`), 253 (InvokeNoNormal `:4018`,
  EstablishActivationClaim `:4968`), 255 (SelectMachine `:3791`, EstablishNativeCodeLayout `:5029`).
  Within each family the tags are distinct. I did not construct two whole units with equal bytes; the
  fixture shows the ambiguity at the operation level.
- Fixture: `identity::operation_family_identity_tests::operations_from_different_families_start_with_different_bytes`.
  Before the fix it failed on `main` at `348268cb`: `assertion left != right failed, left: 42, right: 42`.
- Outcome: **reproduced, fixed.** `encode_operation` now writes a family byte (structural 0, atomic 1,
  calls and effects 2, IEEE float 3, scalar 4, control 5) before the family's own tag. Family tags no longer
  need to be unique across the file, so adding a family arm cannot recreate the collision. The unit identity
  domain moves from `v60` to `v61` because every identity changes.

## Intended

### C1. `ElementViewRead` writes either a place id or `0` plus an argument (record §1 "How it decides")
- Code: `identity.rs:4484-4489`.
- Trace: `semantic_vocabulary` ids are `NonZeroU64` (`semantic_id!`,
  `source-rust/subsystems/compiler/foundation/semantic-records/src/identity.rs:135-145`) and
  `CanonicalBytes::id` writes `get()` as a u64 (`identity.rs:3010-3012`). A place id never writes eight zero
  bytes, so the leading `u64(0)` separates the two forms.
- Outcome: **intended.**

### C3. `ElementViewRead` and `ElementViewWrite` place `path` differently (record §1 "How it decides")
- Code: `identity.rs:4493`, `:4505`. Each arm has its own tag (87, 88) and a fixed field order.
- Outcome: **intended.** No contract asks two operations to share a field order.

### C4. Boolean `StructuralScalarFieldRead` omits the scalar type (record §1 "How it decides")
- Code: `identity.rs:4542-4549`. Tag 11 is written only when `result.scalar_type == ScalarType::Boolean`;
  every other type writes tag 51 and the full `encode_abstract_result`.
- Outcome: **intended.** The tag carries the type.

### C5. `IntegerConstant` and `BooleanConstant` write `result` as a bare id (record §1 "How it decides")
- Code: `identity.rs:4519-4535`. `result` is a `ValueId`, not an `AbstractResult`; `IntegerConstant` writes
  its `scalar_type` field next, and a Boolean constant has only one type.
- Outcome: **intended.**

### C6. Router and family encoders are separate lists; a missed variant panics (record §1 "Cases by shape", five-things item 3)
- Code: router `identity.rs:3440-3523` is an exhaustive match; each family ends with
  `_ => unreachable!(...)` (`identity.rs:4737`, `:5464`).
- Outcome: **intended** (fails closed). A misrouted variant panics on the first identity computation rather
  than hashing nothing. Turning it into a compile error is a code-quality change, not a correctness repair.

### C7. `integer_arithmetic_tag` gives tags to pairs such as `(Saturating, Divide)` (record §1 "Shape", "unsure" item 3)
- Code: `identity.rs:4750-4769`.
- Trace: `IntegerArithmetic::new` admits every policy and operator pair whose obligation shape matches
  (`source-rust/subsystems/compiler/psi-to-native/representations/operation-operands/src/integer_arithmetic.rs`,
  `requires_obligation` and `new`). Terminal Psi has `SaturatingIntegerDivide` and `SaturatingIntegerRemainder`
  (`source-rust/subsystems/interpreter/instructions/execution.rs:4500`).
- Outcome: **intended.** All 15 pairs are constructible and each has its own tag.

### C8. `encode_typed_binary` and `encode_shift` have the same shape (record §1 "Same job elsewhere")
- Code: `identity.rs:4864`, `:4884`. The shift writes a second integer type for the count operand.
- Outcome: **intended** (duplicate shape, different fields). No wrong result.

### C9. A second integer-type encoding lives in `crate::range::encode_integer_type` (record §1 "Same job elsewhere", §2)
- Code: test `width_identities_keep_fixed_bytes_and_distinguish_rigid_parameter_owners` (`identity.rs:5531`)
  pins both encodings and checks that both separate rigid parameter owners.
- Outcome: **intended.** The two encodings feed different identity domains, and the test pins both.

### C10. Nothing in the range versions the byte layout (five-things item 1)
- Code: the unit domain string `omega.psi-optimization-unit-content.v60` (`identity.rs:2672`) prefixes every
  unit identity and versions the layout as a whole. This PR bumps it to `v61`.
- Outcome: **intended.** The version lives outside the range.

### C12. The float-meaning test checks a `Projection` commitment change only at the shared assert (record §3)
- Code: `identity.rs:5593-5619`. The shared `assert_ne!` at `:5619` runs after each arm's mutation, so the
  commitment change is checked.
- Outcome: **intended.**

### C13. Fail-open: none seen (record §1, §2, §3 "Fail-open risks")
- Outcome: **intended.** The only identity risk the record named is C2, now fixed.

## Still open

### C11. No byte-level test for most operation arms (five-things item 5)
- The range holds three tests before this PR. The new fixture covers the family byte for two pairs; the
  roughly 60 arms (an unmeasured estimate in the record) still have no per-arm byte test.
- Outcome: **still open** (test-coverage gap, not a demonstrated wrong result).

## Build state found while testing

- `main` at `348268cb` has two `Proposition::DeclarationIdentity` arms in
  `proposition_encoding::encode_proposition`, added by #5427 and #5434. The second arm is unreachable, and
  `rustc` warns, so `mbx clippy -p optimization-unit --all-targets -- -D warnings` fails on `main`. This PR
  deletes the unreachable second arm; the first arm (tag 23) is unchanged. Open PR #5432 adds the same arm
  again and will need to drop it on rebase.
- `optimization-unit-semantics` does not compile on `main` (`unit_validation.rs:10668`, missing
  `DeclarationIdentity`). Open PR #5449 repairs it; this PR does not duplicate that change.

## Validation

- Before: `mbx test -p optimization-unit --lib operation_family` with only the new test on `main` at
  `348268cb`: 1 failed, `left: 42, right: 42`.
- After: `mbx test -p optimization-unit`: 17 passed, 0 failed.
  `mbx clippy -p optimization-unit --all-targets -- -D warnings`: clean.
- Consumer: see the PR description for `mbx test -p optimization-unit-semantics` with #5449's patch applied
  locally (not committed).
