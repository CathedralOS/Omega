# asm_port_contract

First sample with a port-I/O clobber contract: `asm
clobbers r10, r11, r15, rax, rdx { out 0x3F8, 0x41 }` —
exact register-union contract (order-free), `reaches PortIo`
declares the privileged service on the machine.

Compile-checked only: freestanding target, port I/O is
privileged — run a check target, not native.
