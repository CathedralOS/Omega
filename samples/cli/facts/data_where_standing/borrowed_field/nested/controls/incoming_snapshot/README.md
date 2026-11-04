# Keep a copied reference when its containing pointer changes

A helper saves a reference to the old scalar, replaces `holder.counter`, then
reads the saved reference again. That read must keep the original value and
bound, so the later addition remains safe. This control is unfinished: its
latest completed program run still failed cleanup before either engine ran.

Counter.value <= 60 makes exact i32 70 + value representable without a per-call bound.

This is the incoming-storage control for the
[nested borrowed-field fact](../../README.md), owned by
**CANONICALIZE-SCALAR-RANGE-CONTRACTS**. It uses two pointer crossings:
`Holder.counter` points to `Counter`, and `Counter.value` points to the scalar.
The helper saves the scalar pointer before changing the first pointer.

The [positive](main.omg) replaces 60 with -17 and tests the reverse replacement.
Its saved reads must still produce 130 and 53; retargeting a saved pointer would
make `main` trap. The entry also checks the installed pointers and unchanged
nonzero neighboring fields. It declares exit 0 and binds all four hosted targets.

| Control | Required refusal |
| --- | --- |
| [Missing bound](invalid/main.omg) | Removing only `Counter`'s predicate cannot justify the exact addition. |
| [Wrong identity](invalid_identity/main.omg) | Reading the replacement cannot prove equality to the earlier saved read. |

The program shape, input hashes and authored oracles remain frozen. This
control alone cannot complete the nine-place fact; the full row must run with
native and Terminal agreement and keep its invalid counterparts rejecting.

The [latest completed program run](https://github.com/CathedralOS/Omega/actions/runs/41884089427)
used an optimized compiler built from
`ef422b8dbabf39245587d4752a32033f640d29e9` on Linux x86-64.
The positive reached cleanup but failed with “completion carrier retirement
order”; neither engine ran it. Both invalid controls matched their intended
refusals. This is still a failing positive, not runtime qualification.

[The earlier program run](https://github.com/CathedralOS/Omega/actions/runs/41860338909)
at `98a751f` failed with “the body has no planned scalar completion”.
[The source `218d2fe` attempt](https://github.com/CathedralOS/Omega/actions/runs/41882364283)
stopped during Rust compilation and ran no programs. Their original source
identities and failures remain retained.

[Earlier investigation details](https://github.com/CathedralOS/Omega/blob/218d2feb15d115b3c8fa4d34c55f5346a3a61f7b/samples/cli/facts/data_where_standing/borrowed_field/nested/controls/incoming_snapshot/README.md)
retain the arithmetic, loan, lifetime and cleanup failures under their original
source identities. The [parent note](../../README.md#requirements-and-evidence)
keeps the governing storage/lifetime rules and the remaining acceptance.
