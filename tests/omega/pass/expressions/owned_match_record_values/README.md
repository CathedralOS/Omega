# Record-valued match

`choose` selects either existing whole `Envelope` local or a fresh envelope with
a nested record-valued match. A projected shared getter observes the selected
payload; an unrelated record remains live and is observed afterward. This is not
a test of direct nested local field reads.

The source and canonical Terminal replay controls live in
[`value_dispatch/owned_results.rs`](../../../../../omega-rust/psi/pipeline/05_checked-trees-to-lowered-psi/tests/value_dispatch/owned_results.rs).
The corpus native leg records this case's host build outcome; it is not an
`*_exit` case, so the leg does not execute it.

```text
cargo nextest run -p checked-trees-to-lowered-psi --test value_dispatch --no-fail-fast --no-tests fail -E 'test(owned_results::)'
python3 tools/corpus_gate.py --native --filter expressions/owned_match_record_values
```

A host build does not establish publication or execution for another target.
