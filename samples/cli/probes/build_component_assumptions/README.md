# build_component_assumptions

`Build::accept_component_assumption(builder, "<64-hex digest>")` records the exact
assumption in the selected build. The explicit Build argument precedes the digest;
only the resolved toolchain declaration grants acceptance. The one-argument control
still rejects. This root is check-only; the runnable customer is
`../../independent/component_assumption_recording`.
