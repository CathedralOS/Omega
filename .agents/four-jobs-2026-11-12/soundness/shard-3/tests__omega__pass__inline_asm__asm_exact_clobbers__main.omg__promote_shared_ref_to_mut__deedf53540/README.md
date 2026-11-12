# asm_exact_clobbers

`asm clobbers` is an exact block contract: requires/ensures gate
the block on `main.ready` and the realized out/in register union
{rax, rdx, r10, r11, r15} matches in any declaration order. Port I/O
spells `out port, value` / `in value, port` under `reaches PortIo`.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
