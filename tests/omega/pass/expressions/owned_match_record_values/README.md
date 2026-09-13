# Record-valued match

`choose` selects either existing whole `Envelope` local or a fresh envelope with
a nested record-valued match. A projected shared getter observes the selected
payload; an unrelated record remains live and is observed afterward. This is not
a test of direct nested local field reads.

The source and canonical Terminal replay controls live in
[`value_dispatch/owned_results.rs`](../../../../../omega-rust/psi/pipeline/checked-trees-to-lowered-psi/tests/value_dispatch/owned_results.rs).
Native execution and publication controls live in
[`owned_selection.rs`](../../../../native-differential/tests/scalar_case_results/owned_selection.rs)
and its [`records`](../../../../native-differential/tests/scalar_case_results/owned_selection/records.rs) module.
They cover fresh-first ordering, a saved field before child dispatch, exact
disposal, source receipts, and changed loan origins, offsets, and homes.

```text
cargo nextest run -p checked-trees-to-lowered-psi --test value_dispatch --no-fail-fast --no-tests fail -E 'test(owned_results::)'
cargo nextest run -p omega-native-differential-test --test scalar_case_results --no-fail-fast --no-tests fail -E 'test(owned_selection::)'
```

Publication covers Linux x86-64/AArch64, macOS AArch64, and Windows x86-64.
Native execution runs on matching Linux/macOS hosts; other hosts report a skip.
Cross-publication does not establish execution on an unavailable host.
