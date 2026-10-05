# Returned case payload taken from a state parameter

From the repository root, after preparing this checkout's local package review:

```sh
omega run --both tests/omega/pass/errors/returned_case_payload_from_parameter_exit/main.omg
```

Both engines must exit 70. `Probe::classify` routes to `pc_err`, whose
`kind: Kind` formal is returned inside `Res::Error { kind: kind }`. The payload
is an owned structural parameter, so its consumption owner is the call's own
transfer rather than a result binding this state established.

This pins the source classification, not the construction: the same payload
built from a structural local was already admitted, so only the parameter
spelling regresses if a value planner describes a formal as an operation
sequence local again.
