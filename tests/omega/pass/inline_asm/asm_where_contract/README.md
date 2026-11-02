# asm_where_contract

First sample with a full `asm` block contract:
`requires main.ready`, an order-free `clobbers` union, and
`ensures main.ready` — plus field operands `out main.port,
main.status` / `in main.status, main.port` on a machine
declaring `reaches PortIo`.

Compile-checked only: freestanding target, port I/O is
privileged — run a check target, not native.
