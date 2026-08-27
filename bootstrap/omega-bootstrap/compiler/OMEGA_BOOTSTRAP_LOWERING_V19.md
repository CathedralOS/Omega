# Omega bootstrap guarded full-u64 fixed-buffer lowering, outer version 19

[`OMGCOMP1`](OMEGA_BOOTSTRAP_COMPILATION.md) |
[`OMGRSWA`](OMEGA_BOOTSTRAP_RESOLUTION_V10.md) |
[`CKIR18`](OMEGA_BOOTSTRAP_CHECKED_IR_V18.md)

`OMGLOWJ` version 19 is the focused source/witness lowering relation for the
selected full-width fixed-buffer capability.  Its Delta producer is
[`omega-bootstrap-u64-buffer-to-ckir.alp`](omega-bootstrap-u64-buffer-to-ckir.alp),
not a branch in the historical shared lowerer.

The exact 32-byte little-endian frame header is:

```text
0   8     magic `OMGLOWJ\0`
8   u16   outer version 19
10  u16   minor zero
12  u16   flags zero
14  u16   header size 32
16  u32   exact total frame length
20  u32   exact OMGCOMP1 length
24  u32   exact OMGRSWA length (1,376)
28  u32   resolution selector 10
32  ...   exact OMGCOMP1 || exact OMGRSWA || exact EOF
```

The OMGCOMP component ceiling is 267,280 bytes.  The witness has its exact
selected extent, so the complete frame ceiling is 268,688 bytes.  A wrong
identity/version/selector, cross-pair, truncated component, or trailing byte is
malformed 251; public input exhaustion is 252.  No failure publishes bytes.

The producer validates the complete OMGRSWA header and dense selected tables,
including the paired unit extent, u64 endpoint limbs and authored policy,
projected record/field roles, receiver/parameter/result signatures, block
partition, state/body spans, and five resolved calls.  Every retained span is
bounded by the paired source unit.  Identifier spans contain canonical source
identifiers; machine-call spans begin with the exact target-machine name and
end at `)`.  Thus renamed, reordered, commented and inert-field projections
remain admitted through their rebuilt semantic witness; filenames, labels,
whole-source digests and declaration ordinals are not selectors.

OMGRSWA kind 10 flag one preserves the lookup parameter's exact authored
`u64 in Trapping`.  Lowering consumes that policy only for its selected pure
Less and partial IndexPlace uses and maps the carrier to CKIR18 kind 8 flags
zero.  The fixed array remains authored Trapping source custody.  The length
carrier and the authored exact leaf-plus-literal increment are flag zero.

The append true edge proves `length < N`, so `length + 1` lies in the declared
`0..=N` result interval.  It lowers to opcode 8 Add with ordinary kind-8
operands/result.  CKIR18's carry and result-interval traps are defensive and
unreachable for admitted source; they are not an authored Trapping arithmetic
policy.  Opcode 4 owns dynamic fixed-array bounds trapping.  Opcode 9 remains
pure direct same-carrier Less.  Load, Store and calls retain their inherited
identities.

The exact selected CKIR has two records, four machines (`clear`, `append`,
lookup, entry harness), blocks `1/3/3/1`, five ordinary calls, two kind-8
IndexPlace operations, one kind-8 Add and two kind-8 Less operations.  For the
canonical `N=65,536` source it is 3,924 bytes with SHA-256
`fd468683d3429eebccd700723f5f554ae586b245b7b6a9570caa5b57ed84a9bb`.

This lowering grants no package/build authority, provider installation,
public ABI, native-effect authority, or final Omega-self admission.
