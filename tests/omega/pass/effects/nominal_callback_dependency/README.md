# Nominal callback reach dependencies

From the repository root:

```sh
cargo run -p omega -- --check tests/omega/pass/effects/nominal_callback_dependency/main.omg
cargo nextest run -p package-evidence --test callable_policy reach_dependencies --no-fail-fast --no-tests fail
cargo nextest run -p package-manager --test package_policy_changes generic_reach --no-fail-fast --no-tests fail
cargo nextest run -p checked-trees-to-lowered-psi --lib service_reach_contracts --no-fail-fast --no-tests fail
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

The Terminal regression consumes this same traversal source and adds quiet,
Console-declared, and private-helper Console selections. Each concrete traversal
publishes, decodes, verifies, and interprets to 7 while retaining its selected
service row. Removing the declared contribution or the concrete root row rejects.
Scalar calls use the shared service catalog; inert callbacks retain authored reach
without inventing boundary I/O. This closes concrete-row preservation, not the
remaining portable original-telescope and exact-substitution replay obligation.

The same focused Terminal command also exercises bounded boundary calls through
ordinary helpers. Reload and interpretation retain nominal service parents and
fixed synchronous-invocation reach separately from the exact installation bound,
including overlapping rows. This validates the preselection artifact, not final
provider installation or admission of an unresolved row at a package boundary.
