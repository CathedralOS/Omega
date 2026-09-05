# Remote Package Fixture Pins

Private GitHub mirrors under the `CathedralOS` organization.

Use these exact commits for remote resolver/package-manager tests; do not use
branch names in acceptance tests.

Optional network validation, requiring private `CathedralOS` repository access:

```text
mbx test -p package-manager --test remote_fixtures -- --ignored --test-threads=1
```

| Package | Repository | Exact commit |
| --- | --- | --- |
| `arithmetic-kernels` | `https://github.com/CathedralOS/arithmetic-kernels` | `b65cc9b062f69ef02a586c82cd260d51bf28945c` |
| `generated-table` | `https://github.com/CathedralOS/generated-table` | `cc5fc1addda6aa565f254ad2e002d9e0be189fd4` |
| `host-services` | `https://github.com/CathedralOS/host-services` | `25c18b37f4891aa31b83e1434562fb2ab0994450` |
| `file-journal` | `https://github.com/CathedralOS/file-journal` | `ae37f95cf856d85c05fd4f113a0d32fe6f7229fa` |
| `process-exit` | `https://github.com/CathedralOS/process-exit` | `7926228b0918574dd532dda0008a6aa80881bce9` |
| `network-overreach` | `https://github.com/CathedralOS/network-overreach` | `63657208908572fc8e090f07392682a22d77a518` |
| `remote-journal` | `https://github.com/CathedralOS/remote-journal` | `e8fad1025c0e95df29f44a297aa18a75718f8e95` |
| `axiom-ledger` | `https://github.com/CathedralOS/axiom-ledger` | `8f5bd07f166bcad08842e6bab1ba8b031e3afcb9` |
| `opaque-carrier` | `https://github.com/CathedralOS/opaque-carrier` | `6a29daa655b2aaee0bdb8d85e4a651845e165896` |
| `provider-switchboard` | `https://github.com/CathedralOS/provider-switchboard` | `7011d4e7af9ee06afe5289f880ee37353713aacc` |
| `capability-vault` | `https://github.com/CathedralOS/capability-vault` | `06d9b6688b309ed916eb62d4f3095f9f53c907ff` |
| `graph-workbench` | `https://github.com/CathedralOS/graph-workbench` | `85e962bf5120f84d5f2d8c18c14a6d96d1ec5c64` |

The arithmetic upgrade starts at
`998dac4a03109f67b8c2e87d53ff017007526669` and updates to the table's pin.
Both revisions use current build syntax; the candidate reverses the operands
of commutative addition without changing the public API or authority.

Actual command acceptance (network and matching private-repository credentials
required; HTTPS and SSH run independently):

```text
mbx test -p omega --test package_commands remote::pinned_ssh -- --ignored --test-threads=1
mbx test -p omega --test package_commands remote::pinned_https -- --ignored --test-threads=1
```

These tests install the baseline, update to the candidate, repeat that exact
update, and compile an import through the default alias. The full remote-fixture
matrix checks exact content and complete dependency closures for all table rows,
including `graph-workbench`; no package is skipped for sibling-path dependencies.
It checks the Windows x86-64 compiler target without native emission or execution.

## Filesystem and process authority

`process-exit` starts at `15beea1a49aecce2362e4700e791a46d48bab598`,
removes its process call/reach/invocation at
`13e4afd9c907503cb674d4450fdd3b1a19033d5d`, and restores the original
source at the table's pin. Its repository identity, callable signature, and
host dependency stay fixed. The transition canary requires a separate decision
for both removal and reintroduction; stale or substituted choices do not publish.

`file-journal` upgrades from `3f1e20615b1226aef011b5cfe651a179daca59ad`
to the table's candidate. Only a temporary result binding is removed; its API
and filesystem authority stay unchanged. Both it and `process-exit` pin the
table's `host-services` revision over SSH, rather than using sibling paths
outside their Git trees. Local fixtures retain Path dependencies for offline
tests. Exact remote build overrides live in
[package-remotes](../package-remotes/README.md); other files match locally.

```text
mbx test -p package-manager --test remote_fixtures refreshed_authority_pins -- --ignored --test-threads=1
mbx test -p omega --test package_commands remote_authority::pinned_ssh -- --ignored --test-threads=1
mbx test -p omega --test package_commands remote_authority::transitions:: -- --ignored --test-threads=1
```

These are SSH-closure tests, not independent HTTPS coverage. HTTPS checks above
use the dependency-free arithmetic fixture. Host Git HTTPS credentials and SSH
credentials are independent; neither test silently substitutes the other.

## Transitive authority, assumptions, and opaque data

`graph-workbench` pins the table's arithmetic and filesystem revisions; the
latter contributes `host-services` transitively. `axiom-ledger` exposes an
explicit trust-bearing boundary claim. `opaque-carrier` exposes claim-free
opaque data without a source layout or a fabricated semantic guarantee.

```text
mbx test -p omega --test package_commands remote_review:: -- --ignored --test-threads=1
```

These SSH canaries exercise install and `omega audit packages` using recorded
pins. Audit output reports current findings and does not approve changes or
certify earlier project decisions. The nominal NetworkHost fixtures disclose
reach and invocation but do not contain a concrete network provider.

## Generated process authority

The pure `generated-table` pin in the table remains unchanged. Its separate
`process-authority` fixture branch retains exact commit
`46cb5db4f74d745b735f383d3f22940ac3909c28`, which adds the pinned
`host-services` dependency and generates `terminate` alongside `table_size`.
The build uses only compiler source/output facets; generated runtime code
introduces process authority. Tests use the commit, never the branch selector.

```text
mbx test -p omega --test package_commands remote_authority::generated:: -- --ignored --test-threads=1
```

These SSH tests cover initial review and upgrade from the pure pin, exact
dependency pins, independent dangerous-authority decisions, generated API use
through ordinary checking, and read-only audit output. They do not execute
process termination or claim independent HTTPS closure coverage.

## Named workspace

`library-workbench` is a workspace catalog rather than an importable root
package. Its selected `exact-math` member depends on `integer-constants` in
the same repository. The update changes the latter's implementation; both
reachable members must move together while unrelated repositories stay pinned.

| Repository | Selected package | Baseline commit | Candidate commit |
| --- | --- | --- | --- |
| `https://github.com/CathedralOS/library-workbench` | `exact-math` | `f487125e6fc58d01a2b584424ac5194cdff4f810` | `664d771bbb851201807532e9ed8c444639f65c8f` |

The [local workspace mirror](../package-workspaces/library-workbench/README.md)
matches the candidate. Actual command canaries:

```text
mbx test -p omega --test package_commands workspace::pinned_ssh -- --ignored --test-threads=1
mbx test -p omega --test package_commands generated::pinned_ssh -- --ignored --test-threads=1
```

Each also has an independent `pinned_https` variant requiring HTTPS credentials.

## Same-name source replacement

`library-workbench` retains a separate `same-name-replacement` variant at
`3b597ba19431e504e9fcd3eb9cb74f7566ed865f`. Its `libraries/exact-math`
member declares `arithmetic-kernels` and matches the standalone arithmetic
package's public source with no dependencies. The main workspace is unchanged.

```text
mbx test -p omega --test package_commands source_replacement:: -- --ignored --test-threads=1
```

The SSH canary edits the authored dependency source, then updates. Matching
declared names, aliases, signatures, and empty authority do not bypass the
separate source-replacement decision. Acceptance selects the new repository's
exact named-member pin and checks an import through the unchanged default alias.
