# Package source selection

[Build declarations](../build/declarations.md) define package keys and statically
projected dependency edges. [Acceptance](acceptance.md) is separate from source
resolution. A resolver supplies immutable source custody, not approval of the
package or certification of anything compiled from it.

Dependency projection is hermetic even when later build staging is not. It
extracts one flat unconditional request set from each fetched package without
executing imported code or depending on build-host observations, generated files,
or dependency build output. Close the graph before downloaded build code receives
authority. Each dependency's requests remain unknown until its source is resolved.
The immutable graph is the same for every target; target identity scopes review
and realization, not dependency selection.

## Requester-local graph

A dependency declares its own canonical package name. Default aliases convert
kebab-case to snake_case; `depend_as` supplies a validated exceptional alias for
a real collision. Distinct requesters may use different aliases for the same
key; an ancestor cannot rename dependencies inside another package.

Package-aware compilation consumes a closed validated graph of requester-local
aliases to opaque package keys and canonical source roots. Import discovery does
not combine package-authored dependency rows. Paths route source loading; they
are not nominal identity. Orchestration re-roots each package at exactly its
transitive subgraph and compiles dependency-first.

The Rust focused-file compatibility entrance resolves root-relative and
toolchain imports only; package aliases require the validated package-aware
entrance. It is not the standalone compiler's sealed request protocol.

## Git acquisition and member selection

Acquisition selects a repository and revision. Package selection separately
normalizes to `Root` or `Named(PackageName)` and does not enter `SourceIdentity`.
Several members at one revision therefore share one verified fetch/tree.
Named selection uses only the verified root's statically declared members and
requires exactly one matching package declaration. The selected member path
remains navigation/replay custody and the base for relative dependencies, not
package-key identity.

The resolver owns a syntax-neutral verified-tree session. The Omega-aware
planner supplies bounded member paths after reading the exact root declaration;
the resolver batch-opens their exact declarations and publishes only the
selected subtree. Retain declaration bytes outside that subtree for replay.
All selections in one closure share the same pinned commit/root tree, including
symbolic revisions; do not observe a moving branch separately for each member.

The [resolver security contract](../../../omega-rust/omega/packages/sources/acquisition/SOURCE_RESOLVER_SECURITY.md)
owns locator validation, host-routed Git/SSH, frozen operator-selected Git,
containment, resource controls, and immutable snapshot publication. Host routing
does not change authored lineage. Known adapters may establish namespace
equivalence; generic Git lineage retains transport, user, host, port, path-case,
and suffix distinctions until equivalence is established.

Locks and review consume canonical lineage, exact selected commit/tree/content,
member projection, and immutable snapshot custody, not a fetching-process
receipt. Host credentials never become package authority. Stronger host
sandboxing is deployment policy, not source evidence.

## Reconciliation and updates

The current resolver performs no semantic-version solving. Requests for one key
that select the same immutable instance deduplicate despite different authored
selectors. Different resolutions reject with all conflicting dependency paths;
there is no guessed compatibility relation. Multiple simultaneous instances per
key are unsupported: they would require instance identity throughout types,
conformances, providers, and evidence, not just another alias.

Selective updates preserve pins for unchanged Git locator/revision requests.
Changing an alias or member selection does not refresh the repository. New or
changed requests resolve normally and reconcile with the complete graph.
Selecting a package for update refreshes its repository lineage, not unrelated
transitive repositories. Reachable workspace members and relative Path edges
move together at one revision; this does not add unused members.

Missing preserved content either fails offline or is acquired at the exact
recorded commit, never replaced by a newer selector result. Whole-invocation
offline policy overrides that fetch permission and rejects every new/refreshed
Git request, including transitive discoveries. Local roots remain editable;
local-only graphs need no lock. Resume uses exact proposal pins. Missing old
source restricts source diagnostics, not comparison with the accepted baseline.

Offline policy changes acquisition, not compilation, scoped build outputs,
project decisions, or publication recovery; it is not runtime containment.
The [resolution owner](../../../omega-rust/omega/packages/manager/src/resolution/README.md)
maps these rules to pin and acquisition APIs.
