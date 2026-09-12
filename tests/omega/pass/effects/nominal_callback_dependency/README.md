# Open nominal callback reach dependencies

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

This checks open-generic publication, not original-template retention after
specialization or Terminal dependency replay.
