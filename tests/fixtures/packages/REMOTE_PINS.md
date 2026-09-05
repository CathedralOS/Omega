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
| `process-exit` | `https://github.com/CathedralOS/process-exit` | `15beea1a49aecce2362e4700e791a46d48bab598` |
| `network-overreach` | `https://github.com/CathedralOS/network-overreach` | `d0fe2b00c2485700ace9242114bfa8c8e4a6c526` |
| `remote-journal` | `https://github.com/CathedralOS/remote-journal` | `11a2c6e3825a4a9221fe536164417846f88cd63c` |
| `axiom-ledger` | `https://github.com/CathedralOS/axiom-ledger` | `9f274c21386ea3cd7d7cce5b8d20bcb935f06f58` |
| `opaque-carrier` | `https://github.com/CathedralOS/opaque-carrier` | `4db835ee361b24281ce8be139c5247e6256400a8` |
| `provider-switchboard` | `https://github.com/CathedralOS/provider-switchboard` | `bd0d679da697af139bf3f9dd43d0d84935c7705b` |
| `capability-vault` | `https://github.com/CathedralOS/capability-vault` | `7ee14134ff4756f58f9f1386258066ff794fff5b` |
| `graph-workbench` | `https://github.com/CathedralOS/graph-workbench` | `5d72ed263b69248b3be021f32c71754f27d5f293` |

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
update, and compile an import through the default alias. The remaining remote
mirrors still need their own syntax/content refresh; refreshing selected fixtures does
not establish that the full remote-fixture matrix passes.

## Filesystem and process authority

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
```

These are SSH-closure tests, not independent HTTPS coverage. HTTPS checks above
use the dependency-free arithmetic fixture. Host Git HTTPS credentials and SSH
credentials are independent; neither test silently substitutes the other.

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
