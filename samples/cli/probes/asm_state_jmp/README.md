# asm_state_jmp

First sample with inline assembly: `asm { jmp other() }`
jumps to an Omega state transition — `asm` is a statement
keyword and control-flow asm resolves against the machine's
own states. Bound via bare `main` (free-form entry).

Compile-checked only: no standard-library dependency and no
`Main::exit` — run a check target, not native.
