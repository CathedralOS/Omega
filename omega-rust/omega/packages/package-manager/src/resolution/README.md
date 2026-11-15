# Package resolution

[mod.rs](mod.rs) resolves sources and assembles the complete graph.
[Source selection](../../../../../../wiki/spec/packages/sources.md) owns the
public contract; acquisition keeps immutable trees and approves no package.

[graph/git_pins.rs](graph/git_pins.rs) keeps two policies apart:

- `GitDependencyPins` borrows the accepted source subject and exact update
  keys. Unknown or duplicate selections fail before acquisition. An
  empty selection keeps unchanged requests for install; ordinary unpinned
  resolution updates all selectors. Selected keys refresh their repository
  lineage together.
- `GitResolutionOptions::offline` forbids new/refreshed Git requests across the
  traversal, overriding a kept pin's permission to fetch exactly.

The pins' acquisition option controls only missing pinned revisions; it is
not an offline switch. Refetched objects must match the recorded
commit/tree/content. Alias/member changes do not refresh unchanged
locator/revision requests. A Git edge is never resolved but unfetched:
verifying commit/tree and member declarations already needs the fetched source.

Each workspace declaration keeps the helper source graphs it ran with.
[graph/subject/catalog.rs](graph/subject/catalog.rs) records these apart from
the selected Product graph, checks they share fetched sources, and flattens
nested catalog observations into bounded execution records. Locks encode these
records only when present; other subjects keep their encoding and decision
fingerprints. Pins and locked request admission read both graphs, but catalog
helpers grant no Product dependency or import.

Locked recovery picks the recorded target policy before acquisition, then
rebuilds the exact graph. Resume uses proposed pins, not current branch tips.
Historical-source diagnostics obey the same offline option; missing old bytes
still allow normalized policy comparison. The
[command owner](../../README.md#install-and-update) documents CLI support
and target selection; not every API policy has a command flag.
