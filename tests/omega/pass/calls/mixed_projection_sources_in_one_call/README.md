# Projected arguments from distinct sources

An ordinary call borrows one field through an incoming parameter and another
through a reference returned by a preceding call. The reference-producing
helper completes with a mutable borrow expression, not a transition wrapper.

After local package review:

```sh
omega run --both tests/omega/pass/calls/mixed_projection_sources_in_one_call/main.omg
```

Both engines must execute the unchanged authored program and exit 0.
