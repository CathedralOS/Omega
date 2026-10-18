# data_where_standing

Counter.value <= 60 makes exact i32 70 + value representable without a per-call bound.

The original `main.omg` and `build.omg` remain the compile-only root probe;
`sample.json` records that role for the gates.
The nine child projects add exit-0 native witnesses of the same operation.
Each `invalid/` twin removes only the data `where value <= 60` clause;
construction, transport, call, arithmetic carrier, and operation stay identical.
The unbounded `i32` can exceed `2147483577`, so its addition cannot be proved
representable. The lower carrier endpoint remains safe after adding 70.

| Place | Positive | Reject twin | Evidence transported |
| --- | --- | --- | --- |
| local | `local/main.omg` | `local/invalid/main.omg` | scalar local copied from declared field |
| field | `field/main.omg` | `field/invalid/main.omg` | nested field projection |
| parameter | `parameter/main.omg` | `parameter/invalid/main.omg` | owned Counter parameter |
| call result | `call_result/main.omg` | `call_result/invalid/main.omg` | Counter returned by an ordinary machine |
| match join | `match_join/main.omg` | `match_join/invalid/main.omg` | scalar join from two bounded fields |
| const name | `const_name/main.omg` | `const_name/invalid/main.omg` | named Counter supplied to a const binder |
| another state | `other_state/main.omg` | `other_state/invalid/main.omg` | Counter passed to successor state |
| another machine | `other_machine/main.omg` | `other_machine/invalid/main.omg` | Counter forwarded to another machine |
| another target body | `target_body/main.omg` | `target_body/invalid/main.omg` | four separately checked target bodies |

The generic bodies must justify their additions independently of the entry's
literal value. In particular, the const-name twin must reject before concrete
specialization can conceal a missing standing premise.

Canonical authority is the declared default-domain and delivery contract in
[`dependent_values.md`](../../../../wiki/spec/language/dependent_values.md#default-domains-and-zero-initialization)
and the per-operation representability rule in
[`numeric_values.md`](../../../../wiki/spec/language/numeric_values.md#arithmetic-and-comparisons).
Neither a safe final value nor the caller's literal supplies an unproved
intermediate operation with evidence.

Acceptance command (checking and native execution are reported separately):

```sh
python3 tools/sample_native_gate.py --filter cli/facts/data_where_standing --jobs 2
```

The first semantic refusal on base `55352539edae` was in `const_name`:
`data Counter has default-domain facts whose index-site proof is not implemented`.
All other places checked. The root probe checked unchanged. All nine unbounded
twins rejected their exact addition, including the const generic's body before
specialization. The repair separates structural eligibility from proving each
completed index value's required predicates; it does not infer membership from
canonical bytes or waive data validity.
