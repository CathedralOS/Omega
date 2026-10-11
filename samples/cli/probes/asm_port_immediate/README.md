# asm_port_immediate

An immediate port identity survives checked-assembly lowering into the
final-byte validation record: `out 0x3F8, 0x41` binds the privileged
destination and the closed DX/AL/out envelope.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
