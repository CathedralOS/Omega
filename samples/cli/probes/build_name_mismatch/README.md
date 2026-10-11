# build_name_mismatch

Every name in the dep chain mismatches:

- application declares `app_declared` (directory differs),
- dep directory `depdir`, dep declares `dep_declared`,
- requesting alias `depkit` — and `use depkit::main` resolves.

Together with the `invalid_declared_import` control (declared name does
not satisfy the alias edge), this pins the roles: alias = import name,
declared name = diagnostic identity, directory = on-disk layout.
