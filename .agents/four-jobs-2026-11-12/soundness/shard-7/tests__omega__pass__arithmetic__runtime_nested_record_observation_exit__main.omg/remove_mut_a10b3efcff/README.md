# Nested and sibling computed field observations

One record initializer contains a computed field observation, and the enclosing
scalar expression observes two separate record values. Each receiver must finish
all its initializers before its field is read; neither receiver is a source local.
The result is `7 ^ 21`, or 18. Both engines must exit 70 without output.

Current frontier: the interpreter exits 70; native lowering reaches
`OperationProofUnavailable` for the second division despite retaining the
nonzero entry requirement and the divisor alias. This is a witness for
`CHECKER-DISCHARGES-OPERATION-OBLIGATIONS`, not a request to extend the retiring
Terminal certificate search. The native success oracle deliberately remains red.
`expressions/runtime_nested_record_field_reads_exit` exercises the same receiver
composition without Exact division.

```text
python3 tools/corpus_gate.py --both --filter arithmetic/runtime_nested_record_observation_exit
```
