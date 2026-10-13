# asm_reach_inherit

Ordinary wrappers inherit the helper's declared assembly service:
`helper` declares `reaches MachineControl` for `hlt`, and `main`
propagates that reach without repeating the declaration.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
