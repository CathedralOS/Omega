# Scalar snapshots across invariant restoration

A scalar read as -1 while a mixed predicate is open remains -1 after its parent
is restored, so `consume` accepts that snapshot. Every place copies this fact.

Owner: **CANONICALIZE-SCALAR-RANGE-CONTRACTS** in [TASKS.md](../../../../TASKS.md),
the remaining ordinary arithmetic read-query obligation identified by
[captured storage bounds](../captured_storage_bounds/README.md).
[Invariant windows](../../../../wiki/spec/language/dependent_values.md#invariant-windows)
permit this ordinary scalar read; restoration qualifies the parent's new value,
not an earlier saved scalar. This differs from store-enforced bounds and from
borrowed-storage holes, which have no readable current value.

Each producer captures a valid entry scalar, stores -1 into `Pair.start`, copies
that scalar while `start >= 0 && start <= end` is false, then restores `end = 10`
and `start = 0`. The entry snapshot must still discharge `require_nonnegative`.
The later `consume` independently requires its argument equal -1. No caller
guard on the negative snapshot supplies that proof.

| Place | Positive | Reject twin |
| --- | --- | --- |
| local | [source](local/main.omg) | [false consumer domain](local/invalid/main.omg) |
| field | [source](field/main.omg) | [false consumer domain](field/invalid/main.omg) |
| parameter | [source](parameter/main.omg) | [false consumer domain](parameter/invalid/main.omg) |
| call result | [source](call_result/main.omg) | [false consumer domain](call_result/invalid/main.omg) |
| match join | [source](match_join/main.omg) | [false consumer domain](match_join/invalid/main.omg) |
| const name | [source](const_name/main.omg) | [false consumer domain](const_name/invalid/main.omg) |
| nested machine | [source](other_state/main.omg) | [false consumer domain](other_state/invalid/main.omg) |
| another machine | [source](other_machine/main.omg) | [false consumer domain](other_machine/invalid/main.omg) |
| selected body | [source](target_body/main.omg) | [false consumer domain](target_body/invalid/main.omg) |

Each twin changes only the consumer's input contract from `value == -1` to
`value >= 0`. Its total identity body, producer, restoration and runtime oracle
are unchanged. All nine twins were wrongly accepted on the optimized fixed-base
producer `cf36a5e96fcb69e7fc28963e0d56fe48b915b828`, SHA-256
`b9bcf0d9e08e995a371bbb5ff3682cd3d223e219407150bd409e43b5ad3e649f`.

## Requirements and evidence

- **Establishment:** an admitted parent supplies the initial nonnegative fact;
  an actual store supplies the negative snapshot. A temporarily false default
  predicate cannot justify its own scalar consequence.
- **Identity and lifetime:** the entry values 3 and 5 and saved -1 belong to
  distinct completed reads. Restoring the current field to 0 changes neither
  snapshot. The runtime entry-value guard follows its static nonnegative use.
- **Composition:** all nine carriers retain the same -1 fact and consumer;
  unrestricted identity suppliers preserve their exact actuals. The parameter
  source supplies -1 through its explicit entry contract.
- **Soundness:** the false nonnegative consumer must reject. Store-enforced
  interval bounds remain available; mixed logical consequences require an
  independently established predicate for the exact value considered.
- **Psi 06–08:** ordinary source checking and Terminal lowering retain the
  reads, restoration stores and call requirements. Actual interpreter execution
  agrees with native execution under `omega run --both`.
- **Omega 00–02:** four hosted Build entries, ordinary field layouts and scalar
  call plans carry the values. The selected-body place uses one named provider
  selected explicitly for all four targets.
- **Omega 03–06:** distinct entry values, origins and neighboring hex canaries
  expose wrong homes, offsets and preserved values. The field carrier has its
  own canaries. Both match arms execute and leave distinct markers 11 and 13.
- **Omega 07–09:** direct macOS ARM64 images return -1 and verify restored
  parents before exiting 0. Other-host native execution remains unverified.

The optimized repair producer, SHA-256
`b62468e369d389a0f36a8eb67fb1c2f05c05fecabaad3cd57520c0d1949bcb57`,
executes all nine canonical positives with native/Terminal exits 0/0 and rejects
all nine twins at their consuming obligation. Sources and oracles remained
unchanged throughout each capture. Matching pins live under
`tests/omega/{pass,fail}/facts/invariant_window_snapshot_through_*`.
The PR and claim retain raw observations, exact source hashes, producer inputs,
profiles and independent review. Existing literal-bound, predicate-helper and
mutable-recast programs still execute in both engines.

The parent remains open for scalar-recast source-domain restoration: assigning
`0xFFFFFFFFu32` through `&mut pair.start as &mut u32` without restoring the parent
still passes Source checking on both baseline and repair. Its restored counterpart
executes correctly in both engines. That write-alias/restoration obligation is
separate from this read-premise repair. GUI policy/native qualification and the
broader scalar-range migration also remain open.
