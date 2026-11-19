# build_machine_signature

A [Build entry](../../../../wiki/spec/build/declarations.md#project-role)
is one free machine named `build`. These cases use the entry with a mutable
Build borrow and no additional execution backing. `valid_renamed_param`
passes that borrow through the binding named `context`.

- `invalid_param_missing` and `invalid_extra_param` check the parameter
  roster for this invocation.
- `invalid_param_immutable` tries to declare the application through a shared
  Build reference.
- `invalid_no_build_machine` declares only `setup`.
- `invalid_no_kind` returns without declaring a project role.
