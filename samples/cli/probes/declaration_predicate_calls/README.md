# Declaration predicate calls

Cathedral's joint image/storage geometry classification uses integer embedding
inside a predicate domain. This smaller program also exercises embedding in a
data invariant and reads the classified values natively.

```text
omega run samples/cli/memory/declaration_predicate_calls/main.omg
```

To compare the published native program with the interpreter, run
`mbx xtask sample-gate --filter samples/cli/probes/declaration_predicate_calls` from the repository root.

Expected exit: 0 in both native and Terminal execution. The program exercises
lexically selected mathematical helpers, policy-preserving integer arithmetic,
captured fields, generic carriers, byte predicates and post-call observations.

Compiler `10291dff356` did not select the compiler-owned embedding function in
declaration predicates: their call resolver selected only ordinary machines,
unlike the shared free-call resolver used by machine bodies. The repaired route
uses normal callable selection and the completed-value proof owner; it does not
reevaluate a captured initializer after mutation.

This is a compiler prerequisite, not evidence of a Cathedral boot. Joint root
geometry, foreign backing, stack custody and the firmware handoff remain the
customer's acceptance.
