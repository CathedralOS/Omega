Generic calls infer from argument values, not their underlying storage types.
`make(&backing)` binds `Payload` to `&i32`, consistent with its destination;
`accept(&backing)` binds `Payload` to `i32` without a result-type hint.
Reborrowing a lifetime-tagged reference through `identity` retains its referent
and lifetime instead of creating a reference to its carrier.
The returned linear token retains the original backing, observes 73 and is
consumed exactly once. Native and Terminal execution must both return 0.

Run from the repository root:

```sh
cargo run -p omega -- run --both tests/omega/pass/generics/runtime_borrowed_value_inference_exit/main.omg
```
