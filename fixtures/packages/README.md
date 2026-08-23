# Package Manager Fixture Packages

These are small local packages for `omega-packages` resolver/admission tests.
They intentionally use normal external package names, not `pkg_` prefixes:
package identities, repository names, and lock keys are kebab-case. In-code
dependency aliases use snake_case where Omega identifiers require it.

The first package-manager tests should use these directories through local path
resolution. Once local install/update/audit flows work, mirror the same package
contents into Git repositories under `CathedralOS` and pin tests to exact
commits. Current remote mirror pins are recorded in `REMOTE_PINS.md`.

Packages:

- `arithmetic-kernels`: pure helper/proof baseline with no declared host reach.
- `generated-table`: package-local build input and generated-output fixture.
- `file-journal`: public API intended to reach filesystem authority.
- `network-overreach`: intentionally over-declared public network reach.
- `axiom-ledger`: accepted-claim/deferral admission fixture.
- `provider-switchboard`: provider-selection identity fixture.
- `capability-vault`: capability flow fixture for store/return/acquire/derive.
- `graph-workbench`: root graph fixture depending on pure and capability-bearing
  packages.
