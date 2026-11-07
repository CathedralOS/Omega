# Declaration predicate calls

Cathedral's joint image/storage geometry classification uses integer embedding
inside a predicate domain. This smaller program also exercises embedding in a
data invariant and reads the classified values natively.

```text
omega run --both samples/cli/memory/declaration_predicate_calls/main.omg
```

Expected exit: 0 in both native and Terminal execution. The program exercises
lexically selected mathematical helpers, policy-preserving integer arithmetic,
captured fields, generic carriers, byte predicates and post-call observations.
Its invalid controls under `tests/omega/fail/proofs/declaration_predicate_*`
reject false qualifications, stale observations and shadowed embedding names.

Compiler `10291dff356` did not select the compiler-owned embedding function in
declaration predicates: their call resolver selected only ordinary machines,
unlike the shared free-call resolver used by machine bodies. The repaired route
uses normal callable selection and the completed-value proof owner; it does not
reevaluate a captured initializer after mutation.

This is a compiler prerequisite, not evidence of a Cathedral boot. Joint root
geometry, foreign backing, stack custody and the firmware handoff remain the
customer's acceptance.
