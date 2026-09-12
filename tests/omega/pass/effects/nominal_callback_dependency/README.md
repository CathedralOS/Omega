# Nominal callback reach dependencies

From the repository root:

```sh
cargo run -p omega -- --check tests/omega/pass/effects/nominal_callback_dependency/main.omg
cargo nextest run -p package-evidence --test callable_policy reach_dependencies --no-fail-fast --no-tests fail
cargo nextest run -p package-manager --test package_policy_changes generic_reach --no-fail-fast --no-tests fail
```

The public traversal depends on its nominal Step binder, whose conservative
bound is Console. The package-policy tests consume this source and recover its
canonical policy. Adding `note()` changes the exported dependency from
`reach(Step)` to `Console + reach(Step)` even though the conservative bound stays
Console. Binder renaming, private generic helper extraction, and repeated fixed
contributions preserve canonical identity. Tampered checked dependencies or
forwarded binder selections reject during independent capture.
The package-manager regression changes a private helper and requires an explicit
changed-review decision before proposing and recovering the new lock baseline.

Private quiet and Console applications, reordered applications, a nested relay,
and an unused generic caller preserve the original public traversal and its
canonical policy bytes. Concrete applications use private instances; they do not
consume the authored generic declaration. This checks original-template retention
and package publication, not Terminal dependency replay.
