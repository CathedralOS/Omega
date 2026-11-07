# Ordered terminal call

The first unconditional transition arm returns a factory's ordinary record
result. A later authored arm crashes if executed. Native and Terminal must
both exit 0 and observe the first call's value.

```sh
omega update --project samples/cli/control_flow/ordered_terminal_call --target macos_arm64
omega run --both samples/cli/control_flow/ordered_terminal_call/main.omg
```

The selected call retains its own execution endpoint and return cleanup
coordinate even though a later source arm remains in the checked program.
