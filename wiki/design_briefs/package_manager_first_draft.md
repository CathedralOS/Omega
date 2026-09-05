# Package Manager: Scope and Workflow

This is the short package-manager plan. The
[Build And Package Model](build_and_package_model.md) owns language/build
semantics; [remaining tasks](../../TASKS_PACKAGE_MANAGER.md) own unfinished work.
The [subsystem map](../../omega-rust/omega/packages/README.md) leads to code and
implementation contracts. This document does not duplicate compiler schemas.

## Intent

Provide Cargo-like source dependency install/update without hosting packages.
Start with Git over HTTPS/SSH and local sources. Add other repository/protocol
adapters when there is demand, with explicit source identity and acquisition
rules; an arbitrary URL is not permission to execute a helper.

The additional value is compiler-derived reachability, unsafe API, and
assumption review. Check the complete selected graph before recording acceptance.
Unsupported analysis rejects specifically; missing information never means
“no capabilities.” Native emission is a separate compiler operation.

## Package declaration and identity

The fetched package declares its own name through existing Omega:

```omega
machine build(builder: &mut Build) {
    builder.package("arithmetic-kernels");
}
```

Names use canonical kebab-case. The default import alias replaces hyphens with
underscores: `arithmetic-kernels` becomes `arithmetic_kernels`. Consumers supply
an alias only to override that default. No second manifest, duplicate name
field, magic symbol, or package-specific language syntax is needed.

The stable key combines the declared name with source lineage. The exact
commit/tree/content selection identifies the version of that key. Same-named
packages from unrelated repositories remain different packages; a package's
spelling cannot impersonate another package's boundary trait.

`build.omg` describes the project's source requests, aliases, and build
selections. It does not list trusted capability claims in dependency rows.
The compiler derives capabilities from the fetched source and selected build.

## Dependency planning before build execution

Library `build.omg` files are consumed. Identity, workspace membership, and
dependencies are checked before build execution; they cannot depend on I/O,
generated files, or executing downloaded helpers.

Build orchestration is distinct from effect-free semantic evaluation. The
existing compiler-owned build facets permit scoped package-input reads,
staged-output writes, and logging. They do not expose runtime boundary services,
resolver credentials, or ambient filesystem access. Ordinary code cannot gain
these services by naming a trait or selecting a runtime provider.

Generated source receives final compiler checking and contributes to review.
Installing a library does not let its provider choices override the consuming
root's authority. Reach checks include transitive helpers; a package cannot
hide a dangerous operation behind a falsely narrow declared ceiling.

Use ordinary Omega facilities as the design develops. Do not introduce a new
arithmetic policy, boundary trait, or execution framework unless a concrete use
requires one. Detailed evaluator semantics remain in the build model.

## Authored requests versus accepted lock state

`build.omg` records what the project requests. `omega.lock` records the exact
reconciled graph, selected revisions, normalized accepted capability/API/
assumption baselines, and project decisions for the reviewed targets.

The project trusts whoever lands that lock. It is not a source of trust, a
certificate of package safety, or an auditable proof target. Format checks,
source hashes, and transaction checks detect malformed state, wrong content,
and stale proposals; they cannot prove that the author reviewed anything.

Store enough normalized policy to explain later changes without the old
checkout. A hash alone cannot show a capability diff. The encoding is versioned,
deterministic LF text, not JSON or an opaque whole-policy blob.
[Lock format](../../omega-rust/omega/packages/manager/src/lock/README.md)
belongs to the lock owner.

## Update, install, and missing baselines

1. Resolve the candidate graph and exact sources.
2. Check it with the compiler, including generated source.
3. Compare findings with the accepted lock baseline.
4. Present blocking changes and audit recommendations in the command.
5. After required decisions accept, publish the declaration/lock pair with
   interruption recovery.

An initial install has no accepted policy for new packages. Dangerous authority
or explicit trust-bearing assumptions require review decisions. Completely
checked packages without blocking findings need no approval ceremony.

Upgrades require decisions for capability/assumption changes, including
removals. Public API and source replacement findings remain visible under the
same comparison policy. Retained filesystem, network, or other dangerous
authority recommends audit even when unchanged: the implementation could now
misuse power the project already accepted.

An old source tree helps code review but is not the policy baseline. If it is
unavailable, compare against the lock and recommend standalone candidate audit.
If the lock is absent, perform fresh complete-graph review. Malformed or
unsupported locks fail with recovery guidance, not silent empty acceptance.
Ordinary locked compilation never silently refreshes a branch or tag.

Changing both source and alias may be represented as removal plus addition.
That preserves full review of the new dependency without inventing replacement
pairing from names or declaration order.

## Conflict and audit UX

Review is integrated into install/update, not a Y/N prompt or a separate audit
certification process. The command writes compact compiler-rendered findings
and per-change `pending` decisions. Authors edit those tokens to `accept` or
`reject`, then resume. Changed source or findings invalidate stale decisions;
rejection leaves accepted files unchanged.

Capability triage contains compiler vocabulary and bounded, escaped identifiers.
Package prose, README text, comments, and commit messages do not enter it.
Code diffs are separate hostile input: escaping protects the diff structure,
not an LLM from instructions embedded in source.

Model advice is optional tooling. It can recommend an audit but cannot suppress
compiler findings or resolve project decisions. No built-in model service is
required to install, update, or inspect a package. Whether an audit was serious
is a matter for the people and infrastructure landing the change.

## Compiler checks and ownership

Keep actual checks: invalid proofs and false reach ceilings reject; accepted
assumptions remain explicit; nominal identities prevent boundary spoofing;
transitive and generated authority stays visible. Claim-free opaque boundary
data recommends representation/code audit without pretending to assert a false
proof. More dangerous claims or mechanisms remain separate findings.

Use the earliest checked compiler representation that establishes a fact.
No new Chi stage, native binary, lock certificate, or chain of evidence
promotion is required merely to review a source dependency. Native ABI,
realization, proof replay, and artifact guarantees remain with their compiler
owners. They block a candidate only when that candidate needs an unsupported
guarantee, not the entire package service.

Git and SSH use the operator's normal tooling, keys, agent, configuration, and
host-key policy. Omega owns safe command construction, fetched-source
validation, bounded operation/cleanup, and keeping downloaded code out of
resolver authority. It does not solve desktop ambient authority or attest host
security. Stronger isolation and audit policy belong to the operator.

## Current command surface

```text
omega install <source> [--rev <revision>] [--package <declared-name>] [--as <alias>]
omega update [package-or-alias...] [--to <revision>]
omega install --resume
omega update --resume
omega <install|update> --discard-review
```

These commands exist, including named Git workspace selection, selective pins,
per-target review, and recoverable publication. See the
[command README](../../omega-rust/omega/packages/manager/src/operations/package_commands/README.md)
for options and limitations. Source-code diffs are separate command output;
cache-only recovery of changed local baselines and graph/authority inspection
remain on the task board. Optional offline/model wiring is separate.
Local canaries do not substitute for actual remote acceptance tests.

## Test packages

Use small coherent fixtures for pure arithmetic, filesystem/network authority,
accepted assumptions, boundary spoofing, transitive reach, generated code, and
named workspaces. Their own `build.omg` supplies identity; derive findings through
the real compiler rather than fabricated capability manifests.

[Package fixtures](../../tests/fixtures/packages/README.md) and
[remote pins](../../tests/fixtures/packages/REMOTE_PINS.md) own test setup.
Exercise the actual install/update/import commands. Report missing credentials
or unavailable platforms honestly; do not expand the product into a transport
security framework to make a test pass.
