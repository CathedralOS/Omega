# Aggregate storage address identity

Compatible references to a live Pair expose the same nonzero address without
retaining a loan.

Task: **ADDRESS-EXPOSURE** in [TASKS.md](../../../../TASKS.md).
The [address exposure contract](../../../../wiki/spec/language/counts_and_addresses.md#address-exposure)
requires the referent's address, including when a reference passes through a
machine parameter. An address result carries no borrow or access authority.

Every place copies the sentence above. It obtains an address directly with
`address_of(&value)`, forwards `&value` to `expose`, and compares the two
addresses. It then writes `value.left = 13` and checks the address again. The
field place uses `holder.value` as the same Pair storage. All positives declare
exit 0 and bind the four hosted ProgramEntry targets. Pair explicitly permits
copying so the const-name place can initialize an ordinary mutable local from
its constant value; the constant itself is never given storage by this query.

| Place | Positive | Invalid twin |
| --- | --- | --- |
| local | `local/main.omg` | `local/invalid/main.omg` |
| field | `field/main.omg` | `field/invalid/main.omg` |
| parameter | `parameter/main.omg` | `parameter/invalid/main.omg` |
| call result | `call_result/main.omg` | `call_result/invalid/main.omg` |
| match join | `match_join/main.omg` | `match_join/invalid/main.omg` |
| const name | `const_name/main.omg` | `const_name/invalid/main.omg` |
| another state | `other_state/main.omg` | `other_state/invalid/main.omg` |
| another machine | `other_machine/main.omg` | `other_machine/invalid/main.omg` |
| another target body | `target_body/main.omg` | `target_body/invalid/main.omg` |

Both match arms execute. Each invalid twin changes only the query operand from
a storage reference to the Pair value. It must reject with `storage address_of
requires exactly one ordinary storage reference and no static arguments`.

An additional `controls/exclusive_reference` control holds `&mut value` live
across the shared address operand. It must still reject the overlapping loan.
The row and that control are registered under
`tests/omega/{pass,fail}/facts/aggregate_storage_address_identity_*`.

Reproduce one positive and its control:

```sh
omega run --both samples/cli/facts/aggregate_storage_address_identity/local/main.omg
omega --check samples/cli/facts/aggregate_storage_address_identity/local/invalid/main.omg
```

## Requirements and evidence

- Establishment: initialized Pair storage and ordinary shared-reference formation
  establish every query operand. The invalid twins omit that reference.
- Identity and lifetime: direct, forwarded, and post-write queries name the same
  live storage occurrence. The write demonstrates that the address result does
  not retain a shared loan.
- Composition: the callee observes the supplying referent through its parameter;
  construction results, both match arms, fields and state arrivals preserve the
  same rule. No caller guard establishes referent identity after the call.
- Soundness boundaries: addresses remain numeric observations; the query forms
  no reference from numeric data and grants no mutation authority. The later
  write uses the existing mutable local or parameter.
- Checked-to-Terminal Psi: the checked structural argument preserves the exact
  place and reference crossings in `StorageAddress`. Terminal execution resolves
  original backing without reading fields or constructing a Pair copy.
- Realization and ABI: ordinary aggregate argument/result and shared-reference
  calling plans retain the storage home independently of the addr result.
- Instructions and frames: the existing native address operation uses the
  retained reference geometry and original home, including across `expose`.
- Encoding and emission: native execution and Terminal execution must both exit
  0 for every place; the runtime equality and post-write checks are the oracle.

At main `183d46e5211152b89c937c639e90cbc5ab2a4a67`, the frozen nine positives
all reject solely because non-scalar storage address queries are unimplemented.
The repaired nine positives exit 0 in native Linux x86-64 and Terminal execution;
the nine missing-reference twins and the live-exclusive-reference control reject
for their authored reasons. Producer-bound execution and qualification records
are retained in the worktree's
`build/lane36-evidence` and qualification state. Matching-host Windows,
macOS ARM64 and Linux ARM64 execution is separate from Linux x86-64 evidence.

The broader ADDRESS-EXPOSURE task retains selected-machine applications,
write-only references, and aggregate re-view windows without an exact runtime
storage endpoint. This row establishes ordinary Pair backing through direct
and forwarded references.
