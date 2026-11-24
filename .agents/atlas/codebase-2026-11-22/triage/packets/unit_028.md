# Triage packet: unit_028 (area 12, optimization unit, abstract operations, verified optimization input)

Status: In progress. Early receipts only.

This packet reviews the open correctness observations from
[record unit_028](../../records/unit_028.md) and the matching lines in
[digest 12](../../digests/12-native-representations.md). The baseline is
`main` at `24ad2700685e6cc6062026c87ed5fd24ed415265`. The supplemental range
`OU/identity.rs:4401-5662` belongs to gap packet `unit_028_gap_015` (`second_pass_ranges.tsv`
line 45) and is not reviewed here.

`R/` is `source-rust/subsystems/compiler/psi-to-native/representations/`.
`OU/` = `R/abstract-operations/optimization-unit/src/`, `AO/` =
`R/abstract-operations/src/`, `VOI/` =
`R/abstract-operations/verified-optimization-input/src/`.

## Receipts

| # | Original claim | Current source | Contract | Test or trace | Outcome |
| --- | --- | --- | --- | --- | --- |
| 1 | Record §B2 item 6 and "Five things" item 5; digest 12 line 404: `structural_leaf_copy_pair_pattern` names the absent variant `StructuralCaseLeafCopy` | `AO/operations.rs:1380-1397` | The exported macro matches the leaf-copy variant of `AbstractOperation`. | Merged repair [#5345](https://github.com/CathedralOS/Omega/pull/5345): the macro now names `StructuralLeafCopy`, and its doctest compiles. Ledger row in [07-13](../07-13-psi-and-native.md). | Duplicate (fixed in #5345) |
| 2 | Record §D1; digest 12 line 406: stage 11 uses `BINDER`, so the binding capability is not stage-10-exclusive | `R/bound-framed-machine/issuer/issuer.rs:6`; `pipeline/11_bound-framed-machine-to-selected-form-encoding/src/lib.rs:31-33` | Only stage 10 constructs bound forms. | `grep -rn BINDER source-rust`: the stage 11 hit is a `compile_fail` doctest that proves stage 11 cannot import the issuer. The only real uses are in stage 10 `lib.rs:14-34`. Digest §10 item 5 already marked the claim wrong. | Intended (claim wrong) |
