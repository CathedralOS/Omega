# foreign_binding_forms

First sample enumerating external leaf binding forms:
`ForeignBinding::Syscall(n)`, an evaluated typed
`DllImport` locator (`ForeignBinding<17, 5, 0>`
const-generic result with `MachODylibSymbol`
install-name/symbol), and named `VtableField(field)` —
each `satisfies` clause picks its mechanism, including a
machine call as the via-target. Target-scoped to
macos_arm64; checked every compile.

Compile-checked only: no standard-library dependency and no
`Main::exit` — run a check target, not native.
