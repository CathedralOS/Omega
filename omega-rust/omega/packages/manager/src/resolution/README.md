# Package resolution

[mod.rs](mod.rs) assembles source resolution and the complete graph.
[Source selection](../../../../../../wiki/spec/packages/sources.md) owns the
public contract; acquisition owns immutable tree custody, not package approval.

[graph/resolve/git_pins.rs](graph/resolve/git_pins.rs) separates two policies:

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

Locked recovery selects recorded target policy before touching acquisition, then
reconstructs the exact graph. Resume uses proposed pins, not current branch tips.
Historical-source diagnostics obey the same offline option; missing old bytes
leave normalized policy comparison available. The
[command owner](../operations/package_commands/README.md) documents CLI support
and target selection; the API's policy does not imply every command has a flag.
