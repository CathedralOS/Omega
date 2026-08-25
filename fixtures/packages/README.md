# Package Manager Fixture Packages

These are small local packages for `omega-packages` resolver/admission tests.
They intentionally use normal external package names, not `pkg_` prefixes.
Directory/repository names are kebab-case for clarity, but do not establish
security identity. Each fixture declares its own name through
`const PACKAGE: Package`; the source-qualified `PackageKey`, not the name alone,
is the lock and nominal-symbol identity. Default in-code aliases mechanically
map kebab-case to snake_case.

The first package-manager tests use these directories through local path
resolution. Remote mirrors under `CathedralOS` are pinned in `REMOTE_PINS.md`;
acceptance tests use only those immutable commits.
The integration canary resolves each local closure into immutable source
custody, hands that custody to the package-aware compiler, and asserts the
compiler's canonical package-review projection. This is real review evidence,
not yet sealed lock/admission evidence: no test fabricates a capability
manifest from fixture intent, but the final admission pipeline remains to be
wired.

Packages:

- `arithmetic-kernels`: pure public helper baseline with no declared host reach.
- `generated-table`: package-local build input and generated-output fixture.
- `file-journal`: public API with exact toolchain filesystem reach and
  invocation.
- `process-exit`: public API with exact toolchain `Console` reach and process
  termination authority.
- `network-overreach`: intentionally over-declared public network reach.
- `remote-journal`: retained canonical-filesystem plus package-local network
  reach and invocation.
- `axiom-ledger`: bodyless accepted boundary-claim fixture.
- `opaque-carrier`: claim-free public boundary representation fixture.
- `provider-switchboard`: public clock-service reach/invocation plus exact
  build-owned provider-selection fixture.
- `capability-vault`: capability acquisition/return flow fixture.
- `graph-workbench`: root graph fixture depending on pure and capability-bearing
  packages.

The production-path lineage-spoofing canary constructs two byte-identical
`shared-provider` packages with the same declaration and symbols but distinct
external-local lineages. Both remain separate graph/review identities, and a
root provider selection imported from one cannot be captured by the other.

The admission matrix must additionally cover remote compiler-backed
transport-normalized lineage, provider-selection updates, retained dangerous
authority, missing accepted-lock state, and conflicting instance requests for
one `PackageKey`. Missing old source is already covered with both live and
reopened review-only baselines. `remote-journal` provides the local and remote
source-custody retained-dangerous-authority case; sealed remote admission still
depends on the accepted-lock pipeline.
