# binding_param_capability

`machine archive() reaches Folder` names the caller-provided capability as the
exact boundary-trait requirement `Folder::write_line("archive entry")`; the call
uses that authority without minting its own, and `machine Main::main()` reaches
the same requirement at entry.

Source-visible `Binding<R>` values are retired
(`component_publication.md:198`): a caller-provided capability is the exact
closed application, not a data carrier passed through a parameter.

Compile-checked only: the build declaration is freestanding, so no
`ProgramEntry` is bound.