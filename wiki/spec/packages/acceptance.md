# Package acceptance

The package service resolves and installs source dependencies, checks their
complete supported graph, and compares compiler-derived authority, public API,
and assumptions with project acceptance. Native emission is a separate operation.
[Build declarations](../build/declarations.md) owns graph and package identity.

## Requests and project decisions

`build.omg` records requested sources, aliases, and build selections. `omega.lock`
records exact resolutions, the dependency graph, accepted normalized policy for
reviewed targets, and explicit project decisions. Dependency rows do not contain
package-asserted trusted capability manifests.

The project trusts whoever edits and lands its lock. Parsing, content checks,
graph validation, and transaction consistency detect malformed state, wrong
inputs, and races; they do not authenticate review against that person. A lock
is neither a package-safety certificate nor evidence that anyone audited code.
Project review and branch/build-machine policy remain project responsibilities.

Retained acceptance cannot validate a false proof or suppress actual reachable
authority. Reconstruct compiler findings from selected code; cached analysis is
not an admission certificate. Preserve normalized policy sufficient to explain
changes without the old checkout, not only opaque hashes. The lock uses versioned,
deterministic LF text; [its codec](../../../omega-rust/omega/packages/manager/src/lock/README.md)
owns the physical format.

## Review and publication

1. Resolve the complete candidate and exact sources.
2. Check it, including generated source and transitive authority.
3. Compare findings against the accepted lock baseline.
4. Resolve required decisions for that exact comparison.
5. Publish the declaration/lock pair with interruption recovery.

New dangerous authority or explicit trust-bearing assumptions require acceptance.
Fully checked candidates without blocking findings need no approval ceremony.
Capability/assumption changes, including removals, use the comparison policy;
public API and source replacement remain visible. Unchanged dangerous authority
recommends code audit, not recurring approval: identical permissions do not imply
identical behavior. A source diff is an audit aid, not a security control.

Missing old source limits code review, not comparison against normalized accepted
policy. Recommend standalone candidate audit. Missing lock means fresh complete
review; malformed or unsupported locks reject with recovery guidance rather than
silently becoming an empty acceptance. Locked compilation does not refresh tags
or branches. Source-plus-alias changes may be removal/addition without guessed
replacement pairing.

Editable decisions bind exact compiler findings, not package prose or model
advice. Stale candidates/decisions reject; a rejection leaves accepted files
unchanged. Compiler-rendered triage uses bounded escaped identifiers. Source diffs
remain untrusted input even when their structure is escaped. No model service or
audit attestation is required for the package workflow.

## Authority boundaries

Build execution may perform its admitted scoped effects even if later checking
fails. Package runtime acceptance grants neither build-host nor resolver authority.
Dependencies cannot override consuming-root provider authority. The compiler still
rejects false reach ceilings, invalid proofs, spoofed nominal owners, and missing
analysis; unsupported information is never an empty permission set.

An unbound static machine parameter contributes contractual reach, not an inferred
implementation. Unresolved installation-bound public reach rejects explicitly.
Claim-free opaque boundary data recommends representation/code audit without
inventing a proof claim. Use the earliest checked representation that establishes
each finding; unsupported native guarantees block only candidates needing them,
not all package installation.

Git/SSH use operator tooling and host policy. Omega owns safe acquisition command
construction, fetched-source validation, bounds, and separation from downloaded
code—not host attestation or ambient desktop isolation. Extra compiler/native
receipts belong to their artifacts or concrete caches, not ordinary lock acceptance.

Native preparation compares fresh findings against the same accepted project
baseline. Unchanged accepted policy needs no second native approval file; unmatched
findings use ordinary package review. This does not waive compiler proof checks,
build-execution grants, or independently supplied receiving permission policy.
