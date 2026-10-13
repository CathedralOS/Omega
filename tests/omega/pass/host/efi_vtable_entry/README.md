# efi_vtable_entry

UEFI boundary entry: `Subsystem::EfiApplication` + `builder.freestanding` + `boundary machine Main::main(main, handle: addr, table: &EfiSystemTable)` — multi-param arrival surface; `utf16"..."` literal into `[u16;19]`; `main.out.output_string(con_out, &main.msg)` receiver-spelled Binding call; `invokes TextOutput;` published-invocation ceiling on its own header line; `uefi_x86_64 ... via ForeignBinding::VtableField(output_string)` protocol-field leaf.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
