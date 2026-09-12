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

Unit callbacks must also have an exact closed selection before publication.
The focused Terminal command rejects an unresolved nominal binder as an entry,
executes a closed quiet selection without host effects, and carries two linear
claims through a closed generic ProgramEntry into its selected settlement body.
Direct boundary calls retain both completion receipts; missing, duplicate,
wrong-position, and reordered receipts reject independently. Provider installation
coverage remains in the lowerer's `provider_candidates` integration target, using
explicit boundary calls rather than projecting an unresolved generic binder.

Ordinary and isolated callback producers also replay their existing specialization
commitments before discarding checked custody. Tests replace the template identity,
binder span, selected callback, recorded contract, and argument tuple. Removing
the ordinary nominal callback's final receipt also rejects through its retained
binder-call markers; this is not a universal receipt-omission check for bodies
without those markers. Nested generic calls retain two callback positions interleaved
with type and const binders. This is producer-side replay, not portable proof of
the original generic dependency or a reconstruction of its complete contract.

The same focused Terminal command also exercises bounded boundary calls through
ordinary helpers. Reload and interpretation retain nominal service parents and
fixed synchronous-invocation reach separately from the exact installation bound,
including overlapping rows. This validates the preselection artifact, not final
provider installation or admission of an unresolved row at a package boundary.

For explicit top-level requirements, the focused command also checks fixed
invocation reach in the checked contract envelope and Terminal root-reach
projection, including parent closure and overlap with the normalized bound.
Ordinary helper calls publish, reload, and interpret against newly supplied
host handlers. Boundary calls and installation dependencies retain the same
canonical requirement overload identity; replacing it with a display name
rejects. This preselection interpretation does not establish final provider
installation or permit unresolved rows across ordinary package boundaries.
