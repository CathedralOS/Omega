# foreign_multiregister

libc `lldiv` returns two 64-bit fields in separate Darwin result
registers — the `Division { quotient, remainder }` record arrives as
two i64 slots through `ForeignBinding::DllImport` +
`MachODylibSymbol`.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
