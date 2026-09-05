# Package Manager Fixture Packages

These are small local packages for `package-manager` resolver/review and
current admission tests.
They intentionally use normal external package names, not `pkg_` prefixes.
Directory/repository names are kebab-case for clarity, but do not establish
security identity. Each fixture declares its own name through the ordinary
`builder.package("name")` build surface; the source-qualified `PackageKey`, not
the name alone, is the lock and nominal-symbol identity. Default in-code aliases
mechanically map kebab-case to snake_case.

The first package-manager tests use these directories through local path
resolution. Remote mirrors under `CathedralOS` are pinned in `REMOTE_PINS.md`;
acceptance tests use only those immutable commits.
Mirrors with dependencies use the exact pinned Git declarations in
[package-remotes](../package-remotes/README.md); local copies retain sibling
Path dependencies. Their other files match, and `host-services` has no override.
The integration canary resolves each local closure into immutable source
custody, hands that custody to the package-aware compiler, and asserts the
compiler's canonical package-review projection. No test fabricates a capability
manifest from fixture intent. Under the ratified install/update model, the lock
records pins, the graph, accepted review baselines, and decisions; the project
trusts whoever lands it. A sealed/certified `PackageInstance` or proof of lock
acceptance is not a missing fixture stage. Existing promotion tests describe
current machinery to simplify, while compiler proof/reach and native artifact
checks remain independent of installation.

Packages:

- `arithmetic-kernels`: pure public helper baseline with no declared host reach.
- `generated-table`: package-local Source read plus one-file generated output;
  sponsored review realizes the first bounded `Receipted` grammar.
- `host-services`: ordinary replacement package for exact Console and
  FilesystemHost semantic-binding canaries; its name has no privilege.
- `file-journal`: public API with exact accepted-package filesystem reach and
  invocation.
- `process-exit`: public API with exact accepted-package `Console` reach and process
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

The fixture-derived provider-selection update changes `provider-switchboard`
from `MonotonicClock` to `WallClock` under immutable baseline/candidate custody.
It produces one compiler-owned, opaque-blocking selected-provider-set conflict
and blocks triage.

Real-custody reconciliation coverage creates two actual commits of one declared
Git package, resolves both through immutable Git snapshots, binds them to one
canonical package lineage/key, and verifies closure reconciliation rejects with
both root dependency rows retained as explanation paths.

Dangerous-authority escalation coverage likewise uses two actual commits under
one canonical Git lineage and declared `process-exit` package key. The baseline
keeps the public `Console` parameter inert; the candidate uses the checked-in
process-termination implementation. Compiler comparison emits a changed
blocking callable row and an added blocking process-authority row, update
triage retains the independent process-audit recommendation, and the review
input contains the exact ordinary `main.omg` patch.

The install/update matrix must additionally cover remote compiler-backed
transport-normalized lineage, a missing lock or accepted review baseline, and
conflicting revision requests for one `PackageKey`.
Missing old source is already covered with both live and reopened review-only
baselines. `remote-journal` provides the local and remote source-custody
retained-dangerous-authority case; remote install/update still needs ordinary
lock transaction coverage, not a sealed-admission pipeline. The private-network
canary remains ignored unless the environment has working system GitHub SSH
authentication; missing credentials reject and never trigger a substitute
transport.
