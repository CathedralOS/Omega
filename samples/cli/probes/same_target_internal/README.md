# same_target_internal

An unrealized target's caller resolves its own target's helper: both
`uefi_x86_64` bodies are checked and retained without demanding UEFI
realization on a development host.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
