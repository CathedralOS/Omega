# Static completion cannot discard a linear local

This copies the positive type-selected terminal Unit completion, changing only
`Frame dropped by release_frame` to `Frame [linear]`. The owned local is borrowed,
not consumed or transferred. Both frozen R23 and R24 reject the exact missing
linear disposition. The uncalled cleanup declaration grants no disposal authority.

[Positive and retained evidence](../../../pass/control_flow/runtime_static_literal_terminal_unit_cleanup_exit/README.md).
