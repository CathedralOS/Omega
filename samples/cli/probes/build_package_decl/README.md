# build_package_decl

The positive sample selects a machine from its own `main.omg` through
a file alias. The Build declares no dependency.

- `invalid_missing_module` selects a machine from `dep.omg`, a file absent
  from the package.
- `invalid_nested_entry` declares an explicit `entry() -> i32 { 0 }`
  member inside a machine. The parser reports that this syntax is retired.
