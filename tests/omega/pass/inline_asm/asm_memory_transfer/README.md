# asm_memory_transfer

The contracted AArch64 unordered memory transfers `ldr`/`str` spell their
memory operand as a typed Omega place — `str main.value, main.buffer[0]`
lowers to the same checked assignment the place's own provenance,
permission and exact-type contract carries.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
