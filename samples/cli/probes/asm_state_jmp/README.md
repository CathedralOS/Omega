# asm_state_jmp

Compact control-flow assembly `asm { jmp other() }` is accepted when
the target resolves to an Omega state transition — `asm` is a statement
keyword dispatched contextually.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
