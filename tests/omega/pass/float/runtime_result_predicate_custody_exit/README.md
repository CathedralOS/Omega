# Floating result predicate custody

Run `omega run --both tests/omega/pass/float/runtime_result_predicate_custody_exit/main.omg`.
The authored exit is 0. A selected public result guarantee travels through a
local into another state, through a constructor field into an ordinary call,
and through a detached copy after its source is overwritten. Each route uses
the same IEEE predicate and normal-call snapshot law; no receiving annotation
or callee-body evaluation supplies evidence. Tail-return guarantees describe
the exact completed invocation too.

`fail/float/result_predicate_stale_or_wrong_value` rejects a replaced source and
an unrelated operand. `fail/float/parameter_guarantee_does_not_bound_result`
rejects using a parameter guarantee as a result guarantee.
`fail/float/unbounded_result_does_not_prove_guarantee` rejects a tail result
whose callee publishes no bound. Exact conversion's denotation obligations
remain independently checked by the existing format/integer controls.
