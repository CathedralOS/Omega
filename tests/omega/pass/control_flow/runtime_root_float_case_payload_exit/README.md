# Root float case payloads

On a fresh checkout, prepare the local std dependency with
`omega update --project tests/omega/pass/control_flow/runtime_root_float_case_payload_exit --target macos_arm64`
and accept its review before running.

```sh
omega run --both tests/omega/pass/control_flow/runtime_root_float_case_payload_exit/main.omg
```

Both f32 and f64 case payloads pass through dispatch into successor scalar
parameters and compare with their authored values. The owned sum
is observed directly; no payload copy helper supplies a separate value.
The expected native and Terminal exit is 70.
