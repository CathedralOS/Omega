# Package Manager Fixture Packages

These are small local packages for `omega-packages` resolver/admission tests.
They intentionally use normal external package names, not `pkg_` prefixes.
Directory/repository names are kebab-case for clarity, but do not establish
security identity. Each fixture must ultimately declare its own name through
`const PACKAGE: Package`; the source-qualified `PackageKey`, not the name alone,
is the lock and nominal-symbol identity. Default in-code aliases mechanically
map kebab-case to snake_case.

The first package-manager tests use these directories through local path
resolution; remote mirrors under `CathedralOS` are pinned in `REMOTE_PINS.md`.
Until the compiler implements package declaration extraction, these fixtures
establish only source resolution and data-structure behavior. Tests that
fabricate capability manifests from fixture intent are not package-admission
tests.

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

The admission matrix must additionally cover same-name/different-lineage
spoofing, transport-normalized lineage, retained dangerous authority, missing
old source with a valid lock baseline, missing lock baseline, and conflicting
instance requests for one `PackageKey`.
