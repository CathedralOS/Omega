Run `omega run tests/omega/pass/control_flow/runtime_owned_state_parallel_swap_exit/main.omg --both`
after the ordinary local package review. Both engines exit 70.

A state back edge swaps two owned 13-byte values three times. All source bytes
must be staged before either arrival home changes; the final 4-byte and 1-byte
chunks retain their exact widths. Streaming directly between arrival homes
would overwrite the second input before it was captured.
