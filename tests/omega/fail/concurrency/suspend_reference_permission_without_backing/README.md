# Carry permission does not establish loan preservation

`omega --check tests/omega/fail/concurrency/suspend_reference_permission_without_backing/main.omg`
must reject `inspect_unknown`: an unbounded generic parameter cannot assume
the absence of stored views merely because its value has suspension carry
permission. The concrete reference, aggregate, slice and forwarded-reference
declarations are accepted controls: their exact current invocation-entry
permissions now establish a retained loan frontier. Descriptor carry permission
alone still supplies neither that frontier nor runtime preservation.

The remaining generic refusal is an unresolved frontier, not a language
prohibition on borrowed values across suspension. Runtime execution of concrete
incoming loans is pinned in `pass/concurrency/incoming_loan_after_suspend` and
`pass/concurrency/host_boundary_readiness_exit`.
