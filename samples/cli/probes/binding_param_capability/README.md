# binding_param_capability

`machine archive(folder: Binding<Folder>) reaches Folder` — a
caller-provided capability passed as an ordinary VALUE parameter;
`folder.write_line("...")` uses that authority without minting its own,
and `machine Main::main(folder: Binding<Folder>)` takes the capability
at entry.

Compile-checked only: `core::binding` is depend-free.
