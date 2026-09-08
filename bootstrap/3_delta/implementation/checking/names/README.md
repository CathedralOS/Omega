# Exact-name construction cursors

[`../names.gamma`](../names.gamma) owns ordinary exact-name lookup and insertion.
The helpers here share that sparse-trie representation while retaining a
construction focus across consecutive admitted names. They serve these builders:

- [`../collection.gamma`](../collection.gamma) owns authored-order collection
  and global row provision. It keeps separate type, constructor, and function
  cursors, then finishes their roots into the ordinary global catalog.
- [`../declarations/functions.gamma`](../declarations/functions.gamma) owns
  left-to-right parameter cataloging. It carries a local-name cursor while
  resolving parameter annotations and provisioning active rows, then finishes
  the ordinary trie in the counted environment used by body checking.
- [`../declarations.gamma`](../declarations.gamma) starts from the completed
  census tries. It retains already-complete nullary constructor rows, replaces
  payload constructor and function rows after their declarations resolve, then
  finishes both typed catalogs. Original census roots remain available for
  declaration custody checks throughout that pass.
- [`../types/matches.gamma`](../types/matches.gamma) retains a private cursor
  for each match's exact coverage set. It commits only after binder checks,
  and nested matches retain independent immutable state. Final exhaustiveness
  uses the distinct count; no later phase consumes this working trie.

These builders do not change the name representation delivered to later consumers.

[`cursor.gamma`](cursor.gamma) defines the private cursor and its endpoint and
finish operations. [`cursor_navigation.gamma`](cursor_navigation.gamma) seeks
an exact source name by retaining its shared prefix, climbing departed parents,
and descending existing byte edges. [`cursor_commit.gamma`](cursor_commit.gamma)
inserts an absent terminal or child only after the caller admits that row, or
replaces an existing terminal after resolution without changing child edges.

A cursor contains its focus depth, a source coordinate identifying that prefix,
the ordinary sparse trie at the focus, and a counted immutable ancestor spine.
The ancestors retain complete tagged tries, including terminal options and
sibling order. A seek rebuilds
only departed prefixes; a commit leaves common ancestors deferred. Earlier
cursors and finished roots remain immutable and independently usable.

Seeking does not insert rows. Callers retain their own admission order: global
duplicates precede provision, type provision precedes constructor collection,
and parameter conflicts precede their own annotations and provision. No later
declaration or parameter is examined after failure. Counters, per-type tags,
and exact metadata payloads remain owned by the callers.
Names are compared byte for byte, including prefix endpoints; pair references
never act as truth values or identities. Navigation and long fresh suffixes
use tail loops, not call depth proportional to identifier length.

This is shared construction working state, not a new name representation or a
source sorting pass. General lookup and insertion in `../names.gamma` remain intact.
The change reduces repeated rebuilding when consecutive authored names share
prefixes; arbitrary name order retains the same semantics without promising
the same allocation savings. It does not raise a Gamma bound or convert a raw
evaluator resource failure into a compiler-owned outcome.

The [customer cost investigation](../../../../../wiki/drafts/bootstrap_cost_review.md#delta-simplification)
compares prefix reuse with rebuilding from the root on the exact Epsilon source.
It supports retaining the current implementation without expansion, not claiming
that the customer requires it to fit or that boundary conformance is complete.

Trie tag `0` is absence. Tag `1` carries a terminal option and sparse child
rows. A nonterminal with one edge instead uses `(pair (byte + 2) child)`: one
pair per byte, with no separate absent terminal or singleton child list. Source
identifier bytes cannot collide with the absence or branch tags. Lookup and
ancestor reconstruction read this form directly. Adding a prefix terminal or
sibling expands just that node into the ordinary tag-1 form, preserving its
existing edge and sibling position. Terminal nodes keep tag 1 even when they
have no children. No node deletion or recompression pass is needed.

For a fresh length-`L` name subsequently visited and rebuilt during declaration
resolution, the unary path, ancestor spine, and rebuilt unary path cost one
pair per byte each, rather than the former five, one, and five. Branch and
terminal overhead is separate. This is a path-specific allocation argument,
not a bound on the complete compiler or all admitted source shapes.
