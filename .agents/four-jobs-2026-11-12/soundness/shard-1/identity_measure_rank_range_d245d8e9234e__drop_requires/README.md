# Identity measure rank range

An explicitly selected identity measure must prove the range of its produced
rank, including when the ranked parameter has a storage range. It shares the
ordinary scalar arithmetic proof without losing its declared measure identity.

From the repository root:

```text
cargo run -p omega -- --check --target macos_arm64 tests/omega/pass/termination/identity_measure_rank_range/main.omg
```

This must pass source checking. The paired
[`identity_measure_unbound_name`](../../../fail/termination/identity_measure_unbound_name/main.omg)
must reject: a body that names something other than its parameter is not an
identity measure. This canary does not establish custom-view Terminal
certificate reconstruction or native execution.
