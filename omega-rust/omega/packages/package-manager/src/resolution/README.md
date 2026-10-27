# Package resolution

[mod.rs](mod.rs) assembles source resolution and the complete graph.
[Source selection](../../../../../../wiki/spec/packages/sources.md) owns the
public contract; acquisition owns immutable tree custody, not package approval.

[graph/git_pins.rs](graph/git_pins.rs) separates two policies:

- `GitDependencyPins` borrows the accepted source subject and exact update keys.
  Unknown/duplicate selections reject before acquisition. Empty selection
  preserves unchanged requests for install; ordinary unpinned resolution updates
  all selectors. Selected keys refresh their repository lineage as a unit.
- `GitResolutionOptions::offline` forbids new/refreshed Git requests across the
  whole traversal, overriding preserved pins' exact-fetch permission.

The pins' acquisition option controls only missing preserved revisions; it is not
an offline switch for new or refreshed requests. Reacquired objects must match
recorded commit/tree/content. Alias/member changes do not refresh unchanged
locator/revision requests. Git edges cannot be called resolved-but-unfetched:
commit/tree and member-declaration verification already require resolver custody.

Workspace declaration observations retain the helper source graphs used during
their execution. [graph/subject/catalog.rs](graph/subject/catalog.rs) records
these separately from the selected Product graph, checks shared source custody,
and flattens nested catalog observations into bounded execution records. Locks
encode these records only when present; ordinary subjects retain their existing
encoding and decision fingerprints. Pins and locked request admission read both
graphs, but catalog helpers grant no Product dependency or import.

Locked recovery selects recorded target policy before touching acquisition, then
reconstructs the exact graph. Resume uses proposed pins, not current branch tips.
Historical-source diagnostics obey the same offline option; missing old bytes
leave normalized policy comparison available. The
[command owner](../package_manager/README.md) documents CLI support
and target selection; the API's policy does not imply every command has a flag.
