# Computed range endpoints

An ordinary call supplies a range endpoint before its descriptor is formed.
This program stores that descriptor; `../runtime_call_endpoint_argument_exit`
passes it directly to another call. The sibling `runtime_call_endpoint_order_exit`
observes both endpoint effects running left-to-right exactly once, and
`runtime_call_endpoint_field_transition_exit` transfers a projected view across
a state edge. All four expect exit 70.

From the repository root, with the `omega` CLI built:

```sh
omega update --project tests/omega/pass/slices/runtime_call_endpoint_view_exit
# Review any proposed local package decisions, then omega update --resume
# with the same --project path.
omega run --both tests/omega/pass/slices/runtime_call_endpoint_view_exit/main.omg
```

The corpus gate prepares fixture dependencies directly:

```sh
python3 tools/corpus_gate.py --both --filter runtime_call_endpoint
```

Endpoint computation uses the ordinary value and call schedule. It must not
evaluate a callee body during descriptor planning, duplicate an endpoint call,
or grant access to the backing storage beyond the checked loan.

On macOS ARM64, all four programs execute with exit 70 in both corpus engines.
Standard-library checking now completes through the public CLI as well: an
explicit reborrow preserves its reference's storage origin. On a fresh project,
the command then presents 52 pending package-review decisions for the bundled
standard library. These project trust decisions remain unaccepted; corpus
execution does not establish that this public command has completed.
