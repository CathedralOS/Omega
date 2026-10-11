# build_component_assumptions

`Build::accept_component_assumption(builder, "<64-hex digest>")` is meant
to record the exact component-assumption digest a build accepts. No
authored spelling reaches that behavior today:

- `invalid_assumption_two_arg`: the checker-required `(builder, digest)`
  form reaches the harvester with the builder among the value arguments,
  so it reports "takes exactly one digest spelling".
- `invalid_assumption_one_arg`: dropping the builder to satisfy the
  harvester fails arity — "expects 2 argument(s), got 1".
- Value-qualified `builder.accept_component_assumption(digest)` is
  retired syntax ("use a declaration namespace and explicit arguments").

Filed as COMPONENT-ASSUMPTION-ACCEPTANCE-SHAPE in TASKS.md. The plain
application root beside the controls is the passing coverage.
