# build_package_decl

The positive sample uses `use build_package_decl` to import its own
package-root module without adding a dependency.

- `invalid_dup_kind` declares `Build::application` twice. The refusal
  identifies the repeated role and both supplied application names.
- `invalid_two_kinds` combines `Build::application` and `Build::package`.
  The refusal identifies the mixed roles.
- `invalid_missing_module` imports `invalid_missing_module::dep`, which
  selects no declaration, callable family, or module in the package.
- `invalid_nested_entry` declares an explicit `entry() -> i32 { 0 }`
  member inside a machine. The parser reports that this syntax is retired.
