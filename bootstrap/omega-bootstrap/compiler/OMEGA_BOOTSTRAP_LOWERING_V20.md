# Omega-bootstrap guarded record-array lowering, outer version 20

[`OMGCOMP1`](OMEGA_BOOTSTRAP_COMPILATION.md) |
[`OMGRSWB`](OMEGA_BOOTSTRAP_RESOLUTION_V11.md) |
[`CKIR19`](OMEGA_BOOTSTRAP_CHECKED_IR_V19.md)

`OMGLOWK` version 20 is the focused source/witness lowering relation for the
flat TokenObservation-shaped record-array milestone. Its Delta producer is
`omega-bootstrap-record-array-to-ckir.alp`; no historical shared lowerer is
extended.

The exact 32-byte little-endian frame is:

```text
0   8     magic `OMGLOWK\0`
8   u16   outer version 20
10  u16   minor zero
12  u16   flags zero
14  u16   header size 32
16  u32   exact total frame length
20  u32   exact OMGCOMP1 length
24  u32   exact OMGRSWB length (2,172)
28  u32   resolution selector 11
32  ...   exact OMGCOMP1 || exact OMGRSWB || exact EOF
```

The OMGCOMP ceiling is 267,280 bytes and the complete frame ceiling is 269,484
bytes. Wrong identity/version/selector, a cross-pair, truncation, or trailing
data is malformed 251; public input exhaustion is 252. Failure never publishes
bytes.

The producer validates the full witness header and dense tables, source-unit
pairing, authored `[copy]`, u32/u64/array Trapping policy, record/field types,
machine signatures and blocks, two retained receiver calls, the nine-row
field/parameter store bijection, and nine typed pure literal arguments. All
retained identifier/body/call spans are bounded by the paired source. Call
spans begin at the resolved callee name and end at `)`. Renamed, reordered,
commented, and inert-field source projections therefore lower through rebuilt
semantic rows; filenames, labels, source hashes, and declaration ordinals are
not selectors.

OMGRSWB kinds 2 and 10 retain authored scalar Trapping policy through source
validation. CKIR19 maps their ordinary carriers to its operational u32/u64
types while preserving the selected fixed-array/index bounds behavior. The
true edge proves `count < 16384` for all nine nested IndexPlace/FieldPlace
stores and proves the authored Exact `count + 1`; CKIR defensive bounds,
carry, and interval traps are unreachable for admitted source.

The exact canonical CKIR19 is 6,364 bytes with SHA-256
`eea4a3f85d3abdd452a1622671a42158ae968ec8937113892d7ea35bd32ccb66`.
It has 12 types, 3 records, 13 fields, 3 machines, 10 machine parameters, 7
blocks, 109 operations, 113 operands, 7 terminators, 43 values, and 62 places;
entry machine 2 returns 70.

This lowering grants no build/package authority, provider installation,
public ABI, native effect, or final Omega-self admission.
