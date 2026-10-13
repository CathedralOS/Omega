# asm_reach_wrapper

First sample with a declared assembly service: `machine
helper() reaches MachineControl { asm { hlt } }` — the
ordinary caller inherits the service via the reach contract,
no per-call spelling. Freestanding build (hlt can't run
hosted); `use omega::language::core::assembly` is
depend-free.

Compile-checked only: freestanding target has no hosted exit.
